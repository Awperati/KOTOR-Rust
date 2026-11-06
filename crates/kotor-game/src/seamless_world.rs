//! Seamless areas in the running world: a neighbour joined through a door, the walkmesh of both, and the crossing that
//! makes the neighbour the loaded module without anyone moving. See `docs/SEAMLESS.md`.

use crate::world::{DoorState, Kind, ObjId, World};
use kotor_core::ResType;
use kotor_walkmesh::seamless::{beyond, move_bwm, wrap, Rigid};
use kotor_walkmesh::{parse_bwm, parse_lyt, Bwm, Mover, NavMesh};
use std::collections::{BTreeMap, BTreeSet};

/// A neighbour joined to the loaded module through one of its doors, ready to be walked into.
#[derive(Clone)]
pub struct Attached {
    pub module: String,
    /// Puts the neighbour's own space in ours.
    pub rigid: Rigid,
    /// Our door: where it is, how wide, and the way through it, in our space.
    pub at: [f32; 3],
    pub half: f32,
    pub out: f32,
    /// The neighbour's walkmeshes in its own space, in the order `Sketch::tris` lists their faces.
    pub bwms: Vec<Bwm>,
    /// Faces of ours and of the neighbour's that the other stands on, given up while they are joined.
    pub cut_ours: Vec<usize>,
    pub cut_theirs: Vec<usize>,
    /// This is the module the party has just come from.
    pub behind: bool,
}

/// How the last crossing moved things, for whoever draws: take it once.
#[derive(Clone, Debug)]
pub struct Crossing {
    pub module: String,
    /// Old space to new space.
    pub moved: Rigid,
}

/// Seamless areas in the loaded world. Off, nothing here is used and doors change module as they always did.
#[derive(Default)]
pub struct State {
    pub enabled: bool,
    /// By the link door in this module that leads there.
    pub attached: BTreeMap<ObjId, Attached>,
    pub crossed: Option<Crossing>,
    base: Vec<Bwm>,
    base_module: String,
}

/// How far into the module it has just entered the party must be before the one behind is let go, in metres.
pub const LET_GO: f32 = 30.0;
/// How far past the doorway a character must be for the crossing to count, in metres.
pub const THRESHOLD: f32 = 0.4;

/// `bwms` without the faces numbered in `cut`, counting walkable faces from the first mesh on.
fn without(bwms: &[Bwm], cut: &BTreeSet<usize>) -> Vec<Bwm> {
    let mut n = 0;
    bwms.iter()
        .map(|m| {
            let mut m = m.clone();
            m.faces.retain(|f| {
                if !f.walkable() {
                    return true;
                }
                n += 1;
                !cut.contains(&(n - 1))
            });
            m
        })
        .collect()
}

/// A strip of floor across a doorway, a little each side of it, so that two floors that stop at the door line with a
/// hair between them can be walked across. Its faces are not joined to anything until `rebuild_nav` bridges them.
fn doorway_strip(a: &Attached) -> Bwm {
    let (s, c) = a.out.sin_cos();
    let at = |along: f32, lat: f32| {
        [
            a.at[0] + c * along - s * lat,
            a.at[1] + s * along + c * lat,
            a.at[2],
        ]
    };
    let reach = a.half + 0.3;
    let vertices = vec![
        at(-0.6, -reach),
        at(0.6, -reach),
        at(0.6, reach),
        at(-0.6, reach),
    ];
    let face = |v: [u32; 3]| kotor_walkmesh::Face {
        vertices: v,
        material: 1,
        normal: [0.0, 0.0, 1.0],
        plane_distance: 0.0,
        adjacent: None,
    };
    Bwm {
        kind: 1,
        position: [0.0; 3],
        use_positions: [[0.0; 3]; 4],
        vertices,
        faces: vec![face([0, 1, 2]), face([0, 2, 3])],
        aabbs: Vec::new(),
        edges: Vec::new(),
        perimeters: Vec::new(),
        tree_refit: false,
    }
}

impl World {
    /// The loaded module's walkmeshes, in the order of `Sketch::tris`.
    pub fn room_bwms(&mut self) -> Vec<Bwm> {
        let area = self
            .object(self.area)
            .map(|a| a.tag.clone())
            .unwrap_or_default();
        let Ok(lyt) = self
            .inst
            .read(&area, ResType::LYT)
            .map_err(|e| e.to_string())
            .and_then(|b| parse_lyt(&b).map_err(|e| e.to_string()))
        else {
            return Vec::new();
        };
        lyt.rooms
            .iter()
            .filter_map(|r| parse_bwm(&self.inst.read(&r.model, ResType::WOK).ok()?).ok())
            .collect()
    }

    /// The link door of this module standing at `at`.
    pub fn link_door_at(&self, at: [f32; 3]) -> Option<ObjId> {
        self.objects
            .values()
            .filter(|o| o.kind == Kind::Door && !o.link_module.is_empty())
            .min_by(|a, b| {
                let d = |p: [f32; 3]| (p[0] - at[0]).hypot(p[1] - at[1]);
                d(a.position).total_cmp(&d(b.position))
            })
            .filter(|o| (o.position[0] - at[0]).hypot(o.position[1] - at[1]) < 1.0)
            .map(|o| o.id)
    }

    /// Join a neighbour through `door`: its floor becomes walkable beyond the door, and a door that is opened leads there.
    pub fn seamless_attach(&mut self, door: ObjId, a: Attached) {
        if self.seamless.base_module != self.module || self.seamless.base.is_empty() {
            self.seamless.base = self.room_bwms();
            self.seamless.base_module = self.module.clone();
        }
        self.seamless.attached.insert(door, a);
        self.rebuild_nav();
    }

    pub fn seamless_detach(&mut self, door: ObjId) {
        if self.seamless.attached.remove(&door).is_some() {
            self.rebuild_nav();
        }
    }

    /// The navigation mesh of the loaded module and everything joined to it, with closed doors blocked.
    fn rebuild_nav(&mut self) {
        let cut: BTreeSet<usize> = self
            .seamless
            .attached
            .values()
            .flat_map(|a| a.cut_ours.iter().copied())
            .collect();
        let mut meshes = without(&self.seamless.base, &cut);
        for a in self.seamless.attached.values() {
            let theirs: BTreeSet<usize> = a.cut_theirs.iter().copied().collect();
            meshes.extend(
                without(&a.bwms, &theirs)
                    .iter()
                    .map(|m| move_bwm(m, &a.rigid)),
            );
        }
        if meshes.is_empty() {
            return;
        }
        meshes.extend(self.seamless.attached.values().map(doorway_strip));
        let mut nav = NavMesh::from_meshes(&meshes);
        for a in self.seamless.attached.values() {
            let (s, c) = a.out.sin_cos();
            let at = |along: f32, lat: f32| {
                [
                    a.at[0] + c * along - s * lat,
                    a.at[1] + s * along + c * lat,
                    a.at[2],
                ]
            };
            // The floors on the two sides may stop a little short of the door line or run a little past it, so each
            // side is joined at the first floor found going out from the door.
            let mut k = -(a.half / 0.75).floor();
            while k * 0.75 <= a.half {
                let lat = k * 0.75;
                let pair = {
                    let find = |sign: f32| {
                        (1..=30).map(|i| sign * i as f32 * 0.1).find(|&d| {
                            let p = at(d, lat);
                            nav.height_at(p[0], p[1], p[2] + 0.5).is_some()
                        })
                    };
                    (find(-1.0), find(1.0))
                };
                if let (Some(before), Some(after)) = pair {
                    let portal = (at(0.0, lat - 0.4), at(0.0, lat + 0.4));
                    // Through the strip, which covers the gap the two floors leave.
                    nav.bridge(at(before, lat), at(0.0, lat), portal);
                    nav.bridge(at(0.0, lat), at(after, lat), portal);
                }
                k += 1.0;
            }
        }
        for o in self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door && !o.open)
        {
            let (p, q) = crate::play::door_line(o);
            nav.block_line(p, q);
        }
        self.nav = Some(nav);
    }

    /// Cross into the neighbour joined through `door`: the world becomes the neighbour's, and everything that stands in
    /// ours is carried over by the same move, so that nobody is anywhere new. The module left behind stays joined until
    /// the party is well clear of it.
    fn seamless_cross(&mut self, door: ObjId) {
        let Some(a) = self.seamless.attached.remove(&door) else {
            return;
        };
        let inv = a.rigid.inverse();
        let who: Vec<ObjId> = std::iter::once(self.pc)
            .chain(self.party.iter().copied())
            .collect();
        let before: Vec<(ObjId, [f32; 3], f32)> = who
            .iter()
            .filter_map(|id| self.objects.get(id).map(|o| (*id, o.position, o.facing)))
            .collect();
        let heading = self.pc_mover.heading;
        let old_base = if self.seamless.base_module == self.module {
            std::mem::take(&mut self.seamless.base)
        } else {
            self.room_bwms()
        };
        let old_module = self.module.clone();
        self.seamless.attached.clear();
        if let Err(e) = self.transition(&a.module, "") {
            self.errors
                .push(format!("seamless crossing to {}: {e}", a.module));
            return;
        }
        self.followers.clear();
        for (id, p, facing) in before {
            if let Some(o) = self.objects.get_mut(&id) {
                o.position = inv.apply(p);
                o.facing = facing + inv.yaw.to_degrees();
            }
        }
        let pc = self.pc;
        let at = self.objects.get(&pc).map_or([0.0; 3], |o| o.position);
        self.pc_mover = Mover::new(at, wrap(heading + inv.yaw));
        self.seamless.base = a.bwms.clone();
        self.seamless.base_module = a.module.clone();
        // The neighbour's own door, now standing where ours was: open, because the party has just come through it.
        let there = inv.apply(a.at);
        if let Some(d2) = self.link_door_at(there) {
            if let Some(o) = self.objects.get_mut(&d2) {
                o.open = true;
                o.door = DoorState::Open;
            }
            let back_out = wrap(a.out + inv.yaw + std::f32::consts::PI);
            let (half, at) = self
                .objects
                .get(&d2)
                .map_or((a.half, there), |o| (o.door_half_width, o.position));
            self.seamless.attached.insert(
                d2,
                Attached {
                    module: old_module,
                    rigid: inv,
                    at,
                    half,
                    out: back_out,
                    bwms: old_base,
                    cut_ours: a.cut_theirs.clone(),
                    cut_theirs: a.cut_ours.clone(),
                    behind: true,
                },
            );
        }
        self.rebuild_nav();
        self.seamless.crossed = Some(Crossing {
            module: a.module,
            moved: inv,
        });
    }

    /// Cross when the player has walked through an open door that has a neighbour joined, and let go of the module left
    /// behind once the party is well clear of it. Called every step.
    pub(crate) fn update_seamless(&mut self) {
        if self.seamless.attached.is_empty() {
            return;
        }
        let pos = self.pc_mover.pos;
        let through = self.seamless.attached.iter().find_map(|(id, a)| {
            let open = self.objects.get(id).is_some_and(|d| d.open);
            let (along, across) = beyond(a.at, a.out, pos);
            (open && along > THRESHOLD && across <= a.half + 0.5).then_some(*id)
        });
        if let Some(id) = through {
            self.seamless_cross(id);
            return;
        }
        let who: Vec<[f32; 3]> = std::iter::once(self.pc)
            .chain(self.party.iter().copied())
            .filter_map(|id| self.objects.get(&id).map(|o| o.position))
            .collect();
        let gone: Vec<ObjId> = self
            .seamless
            .attached
            .iter()
            .filter(|(_, a)| a.behind && who.iter().all(|p| beyond(a.at, a.out, *p).0 < -LET_GO))
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            self.seamless_detach(id);
        }
    }
}
