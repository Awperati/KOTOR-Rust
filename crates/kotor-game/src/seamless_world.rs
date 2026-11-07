//! Seamless areas in the running world: a neighbour joined through a door, the walkmesh of both, and the crossing that
//! makes the neighbour the loaded module without anyone moving. See `docs/SEAMLESS.md`.

use crate::world::{DoorState, Kind, ObjId, World};
use kotor_core::ResType;
use kotor_walkmesh::seamless::{beyond, move_bwm, wrap, Rigid};
use kotor_walkmesh::{parse_bwm, parse_lyt, Bwm, Mover, NavMesh};
use std::collections::{BTreeMap, BTreeSet};

/// A creature standing in a module, as much as is needed to draw it where it stands.
#[derive(Clone, Debug)]
pub struct Figure {
    pub appearance: i32,
    pub template: String,
    /// The armour it wears: its base item and template.
    pub armour: Option<(i32, String)>,
    pub at: [f32; 3],
    pub heading: f32,
    /// The name of the underwear it wears under its armour, as the object has it.
    pub underwear: String,
    /// Its tag, and the waypoints named after it (`WP_<tag>_01`, `_02`...) in order: the route it walks.
    pub tag: String,
    pub route: Vec<[f32; 3]>,
}

/// What stands in a module besides its floor, in its own space, for drawing it before the party gets there: its
/// creatures and its doors (template and position). Still, not simulated.
#[derive(Clone, Debug, Default)]
pub struct Scenery {
    pub area: String,
    pub figures: Vec<Figure>,
    pub doors: Vec<(String, [f32; 3])>,
}

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
    /// Its creatures and doors, for drawing.
    pub scenery: Scenery,
    /// The two modules overlap in plan. Each is walkable only near the door (`far_*` are the faces left out) and the
    /// other is drawn through the doorway alone, never as a neighbour.
    pub portal: bool,
    pub far_ours: Vec<usize>,
    pub far_theirs: Vec<usize>,
    /// Floor missing before and after the doorway line, in metres: the strip across the doorway covers it.
    pub gap_before: f32,
    pub gap_after: f32,
}

impl Attached {
    /// Whether its picture is drawn only through the doorway. A portal's is. So is that of a door with a pocket behind it on
    /// either side: the other module stands where the pocket is, and drawn together the two floors lie in one plane and fight
    /// for the pixels, and each one's walls stand in the other's rooms.
    pub fn seen_through_doorway(&self) -> bool {
        self.portal || !self.cut_ours.is_empty() || !self.cut_theirs.is_empty()
    }
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
    /// The clock the neighbours' walkers go by, put in by whoever draws, and the way each one faces now (by module and
    /// place in its `Scenery::figures`), so that the crossing can leave them as they stood.
    pub clock: f32,
    pub facing: BTreeMap<(String, usize), f32>,
    base: Vec<Bwm>,
    base_module: String,
}

impl State {
    /// Drop everything joined and any crossing not yet taken, for a world that is loaded afresh.
    pub fn forget_neighbours(&mut self) {
        self.attached.clear();
        self.crossed = None;
        self.facing.clear();
        self.base.clear();
        self.base_module.clear();
    }
}

/// How near a door a script must find the player to send them through it as a crossing, in metres.
const EXIT_RANGE: f32 = 4.0;

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
    let (back, front) = (a.gap_before + 0.6, a.gap_after + 0.6);
    let vertices = vec![
        at(-back, -reach),
        at(front, -reach),
        at(front, reach),
        at(-back, reach),
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

    /// The creatures (not the party) and doors of the loaded module, where they stand.
    pub fn scenery(&self) -> Scenery {
        let area = self
            .object(self.area)
            .map(|a| a.tag.clone())
            .unwrap_or_default();
        let mut s = Scenery {
            area,
            ..Scenery::default()
        };
        let waypoints: Vec<(String, [f32; 3])> = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Waypoint)
            .map(|o| (o.tag.to_ascii_lowercase(), o.position))
            .collect();
        let route_of = |tag: &str| -> Vec<[f32; 3]> {
            let prefix = format!("wp_{}_", tag.to_ascii_lowercase());
            let mut r: Vec<(u32, [f32; 3])> = waypoints
                .iter()
                .filter_map(|(t, p)| {
                    let n = t.strip_prefix(&prefix)?.parse::<u32>().ok()?;
                    Some((n, *p))
                })
                .collect();
            r.sort_by_key(|(n, _)| *n);
            r.into_iter().map(|(_, p)| p).collect()
        };
        for o in self.objects.values() {
            match o.kind {
                Kind::Creature if !o.dead && o.id != self.pc && !self.party.contains(&o.id) => {
                    let armour = o
                        .equipped
                        .get(&1)
                        .and_then(|i| self.object(*i))
                        .map(|i| (i.base_item, i.template.clone()));
                    s.figures.push(Figure {
                        appearance: o.appearance,
                        template: o.template.clone(),
                        armour,
                        at: o.position,
                        heading: o.facing.to_radians(),
                        underwear: o.underwear.clone(),
                        tag: o.tag.clone(),
                        route: route_of(&o.tag),
                    });
                }
                Kind::Door => s.doors.push((o.template.clone(), o.position)),
                _ => {}
            }
        }
        s
    }

    /// The link door or link trigger of this module standing at `at`: a trigger stands at the middle of its polygon.
    pub fn link_door_at(&self, at: [f32; 3]) -> Option<ObjId> {
        let place = |o: &crate::world::Object| -> [f32; 3] {
            if o.kind == Kind::Trigger && o.polygon.len() >= 3 {
                let n = o.polygon.len() as f32;
                [
                    o.polygon.iter().map(|p| p[0]).sum::<f32>() / n,
                    o.polygon.iter().map(|p| p[1]).sum::<f32>() / n,
                    o.position[2],
                ]
            } else {
                o.position
            }
        };
        self.objects
            .values()
            .filter(|o| matches!(o.kind, Kind::Door | Kind::Trigger) && !o.link_module.is_empty())
            .map(|o| (o, place(o)))
            .min_by(|a, b| {
                let d = |p: [f32; 3]| (p[0] - at[0]).hypot(p[1] - at[1]);
                d(a.1).total_cmp(&d(b.1))
            })
            .filter(|(_, p)| (p[0] - at[0]).hypot(p[1] - at[1]) < 1.0)
            .map(|(o, _)| o.id)
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

    /// Whether a closed door of the loaded module blocks anyone. Where two modules overlap, the one just entered may have
    /// doors standing in the part of it that lies beyond the doorway, which is where the module behind is walked: those
    /// are not in the way there.
    pub(crate) fn in_the_way(&self, o: &crate::world::Object) -> bool {
        !self.beyond_the_portal(o.position)
    }

    /// Whether `p` lies in the part of the loaded module that is beyond a doorway to a module it overlaps.
    pub(crate) fn beyond_the_portal(&self, p: [f32; 3]) -> bool {
        self.seamless
            .attached
            .values()
            .any(|a| a.portal && a.behind && beyond(a.at, a.out, p).0 > THRESHOLD)
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
            let theirs: BTreeSet<usize> =
                a.cut_theirs.iter().chain(&a.far_theirs).copied().collect();
            meshes.extend(
                without(&a.bwms, &theirs)
                    .iter()
                    .map(|m| move_bwm(m, &a.rigid)),
            );
        }
        if meshes.is_empty() {
            return;
        }
        // The strips come last, so the faces from this count on are theirs.
        let real: usize = meshes
            .iter()
            .map(|m| m.faces.iter().filter(|f| f.walkable()).count())
            .sum();
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
            let mut found: Vec<(f32, f32, f32)> = Vec::new();
            while k * 0.75 <= a.half {
                let lat = k * 0.75;
                let find = |sign: f32| {
                    let steps = ((a.gap_before.max(a.gap_after) + 3.0) / 0.1) as i32;
                    (1..=steps).map(|i| sign * i as f32 * 0.1).find(|&d| {
                        let p = at(d, lat);
                        nav.locate_if(p[0], p[1], p[2] + 0.5, &|i| i < real)
                            .is_some()
                    })
                };
                if let (Some(before), Some(after)) = (find(-1.0), find(1.0)) {
                    found.push((lat, before, after));
                }
                k += 1.0;
            }
            // The big faces on each side are the same ones all along the doorway, so they are joined once, and the join
            // covers the whole stretch where floor was found on both sides: everyone crosses where they like, not through
            // one narrow gap.
            let (Some(lo), Some(hi)) = (
                found.iter().map(|f| f.0).reduce(f32::min),
                found.iter().map(|f| f.0).reduce(f32::max),
            ) else {
                continue;
            };
            let portal = (at(0.0, lo - 0.4), at(0.0, hi + 0.4));
            let (floor, strip) = (&|i: usize| i < real, &|i: usize| i >= real);
            for (lat, before, after) in found {
                // Straight across, which is the way a path takes; the strip is the floor under it.
                nav.bridge_between(at(before, lat), at(after, lat), portal, floor, floor);
                nav.bridge_between(at(before, lat), at(0.0, lat), portal, floor, strip);
                nav.bridge_between(at(0.0, lat), at(after, lat), portal, strip, floor);
            }
        }
        for o in self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door && !o.open && self.in_the_way(o))
        {
            let (p, q) = crate::play::door_line(o);
            nav.block_line(p, q);
        }
        self.nav = Some(nav);
    }

    /// Cross into the neighbour joined through `door`: the world becomes the neighbour's, and everything that stands in
    /// ours is carried over by the same move, so that nobody is anywhere new. The module left behind stays joined until
    /// the party is well clear of it.
    /// The door of the module joined to `module` that the player stands at, if a script is sending them there: close
    /// enough to it to be coming through it.
    pub(crate) fn seamless_exit_to(&self, module: &str) -> Option<ObjId> {
        let p = self.pc_mover.pos;
        self.seamless
            .attached
            .iter()
            .find(|(_, a)| {
                !a.behind
                    && a.module.eq_ignore_ascii_case(module)
                    && (a.at[0] - p[0]).hypot(a.at[1] - p[1]) < EXIT_RANGE
            })
            .map(|(d, _)| *d)
    }

    pub(crate) fn seamless_cross(&mut self, door: ObjId) {
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
        let mut old_scenery = self.scenery();
        // The creatures of the module left are real and stand where they stand: they walk no route in the picture of it.
        for f in &mut old_scenery.figures {
            f.route.clear();
        }
        // Who was walking in the module entered, and where, for putting them there once it is loaded.
        let walkers: Vec<(Figure, crate::seamless_walk::Pose)> = a
            .scenery
            .figures
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.route.is_empty())
            .map(|(i, f)| {
                let mut p = crate::seamless_walk::pose(f, self.seamless.clock);
                if let Some(h) = self.seamless.facing.get(&(a.module.clone(), i)) {
                    p.heading = *h;
                }
                (f.clone(), p)
            })
            .collect();
        self.seamless.attached.clear();
        if let Err(e) = self.transition(&a.module, "") {
            self.errors
                .push(format!("seamless crossing to {}: {e}", a.module));
            return;
        }
        self.followers.clear();
        // Each walker stands where the picture showed it, not where its file has it.
        for (f, p) in walkers {
            let near = |o: &crate::world::Object| {
                o.kind == Kind::Creature
                    && o.tag.eq_ignore_ascii_case(&f.tag)
                    && (o.position[0] - f.at[0]).hypot(o.position[1] - f.at[1]) < 0.75
            };
            let id = self.objects.values().find(|o| near(o)).map(|o| o.id);
            if let Some(o) = id.and_then(|id| self.objects.get_mut(&id)) {
                o.position = p.at;
                o.facing = p.heading.to_degrees();
            }
        }
        for (id, p, facing) in before {
            if let Some(o) = self.objects.get_mut(&id) {
                o.position = inv.apply(p);
                o.facing = facing + inv.yaw.to_degrees();
            }
        }
        let pc = self.pc;
        let at = self.objects.get(&pc).map_or([0.0; 3], |o| o.position);
        self.pc_mover = Mover::new(at, wrap(heading + inv.yaw));
        // Modern movement keeps a body with momentum, a roll or a fall under way: it goes on across the crossing.
        self.pc_body.carry(inv.yaw, inv.shift);
        self.body_mark = at;
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
                .filter(|o| o.kind == Kind::Door)
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
                    scenery: old_scenery,
                    portal: a.portal,
                    far_ours: a.far_theirs.clone(),
                    far_theirs: a.far_ours.clone(),
                    gap_before: a.gap_after,
                    gap_after: a.gap_before,
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
            // A trigger has no door to open: walking through is the crossing.
            let open = self
                .objects
                .get(id)
                .is_some_and(|d| d.open || d.kind == Kind::Trigger);
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

#[cfg(test)]
mod tests {
    use crate::{MovementMode, World};
    use kotor_archive::Install;

    /// A player in the air as the crossing comes, in Modern movement, comes down on the new floor and is not flung.
    #[test]
    fn a_fall_that_is_under_way_at_the_crossing_lands_on_the_new_floor() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let mut w = World::new(Install::open(&dir).unwrap()).unwrap();
        w.load_module("tar_m02ac").unwrap();
        let sketch = w.sketch();
        let site = sketch
            .sites
            .iter()
            .find(|s| s.tag == "Apartments")
            .unwrap()
            .clone();
        let (att, _) = crate::seamless::prepare(&dir, &sketch, &site, &Default::default()).unwrap();
        let door = w.link_door_at(site.at).unwrap();
        let (rigid, out) = (att.rigid, att.out);
        w.seamless_attach(door, att);
        w.use_door(door);
        for _ in 0..90 {
            w.step(1.0 / 60.0);
        }
        w.movement = MovementMode::Modern;
        let pc = w.pc;
        w.teleport(
            pc,
            [
                site.at[0] - out.cos() * 4.0,
                site.at[1] - out.sin() * 4.0,
                site.at[2],
            ],
        );
        w.step(1.0 / 60.0);
        let floor = rigid.inverse().apply(site.at)[2];
        let (mut dropped, mut crossed) = (false, false);
        for _ in 0..300 {
            w.steer = Some(([out.cos(), out.sin()], true));
            let p = w.pc_mover.pos;
            let near = (p[0] - site.at[0]).hypot(p[1] - site.at[1]);
            if !dropped && w.module == "tar_m02ac" && near < 1.6 {
                // Lifted a metre and a half, so that it falls across the line.
                w.pc_body.pos[2] += 1.5;
                w.pc_body.grounded = false;
                w.pc_mover.pos[2] += 1.5;
                w.body_mark = w.pc_mover.pos;
                dropped = true;
            }
            w.step(1.0 / 60.0);
            crossed |= w.module == "tar_m02aa";
        }
        assert!(dropped && crossed);
        assert!(w.pc_body.grounded, "landed");
        assert!(
            (w.pc_mover.pos[2] - floor).abs() < 0.5,
            "on the floor of the new module: {} against {floor}",
            w.pc_mover.pos[2]
        );
    }
}
