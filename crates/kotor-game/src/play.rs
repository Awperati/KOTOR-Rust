//! Interaction and movement on top of the world: picking, click-to-walk, doors, triggers, module
//! transitions and the per-step update.

use crate::follow::{dist2d, slide_around, trim_route};
use crate::movement::{Body, Ground, Input, MovementMode};
use crate::world::{DoorState, Kind, ObjId, Object, World};
use kotor_core::{Error, ResType, Result};
use kotor_script::OBJECT_INVALID;
use kotor_walkmesh::{parse_bwm, parse_lyt, Bwm, MoveParams, MoveStatus, Mover, NavMesh};
use std::collections::HashSet;

/// Half the width a closed door blocks when its model gives none (metres).
pub const DOOR_BLOCK_RADIUS: f32 = 1.1;
/// The line across its doorway that a closed door blocks.
pub(crate) fn door_line(o: &Object) -> ([f32; 3], [f32; 3]) {
    let ([dx, dy], h) = (o.door_axis, o.door_half_width);
    let (x, y, z) = (o.position[0], o.position[1], o.position[2]);
    ([x - dx * h, y - dy * h, z], [x + dx * h, y + dy * h, z])
}
/// How close the player must get before using a door, talking to a creature, or using a placeable.
pub const DOOR_RANGE: f32 = 1.6;
/// Seconds a door takes to open or close. Measured: `opening1` and `closing1` of door model DOR_LHR01 last 1.0 s, and
/// the reference video agrees, see `docs/REFERENCE.md`.
pub const DOOR_ANIM_SECONDS: f32 = 1.0;
pub const TALK_RANGE: f32 = 1.8;
pub const USE_RANGE: f32 = 1.6;
/// The row of `skills.2da` for Security.
const SKILL_SECURITY: usize = 6;
/// Running speed: `runrate` of `creaturespeed.2da`, which is also `rundist` (3.96 m) over the 0.733 s run cycle.
/// The reference video gives 5.8 m/s, give or take 0.6: see `docs/REFERENCE.md`.
pub const RUN_SPEED: f32 = 5.4;
/// When steered, the player keeps this far from the edge of the walkmesh, so that it does not stand in a wall.
const STEER_CLEARANCE: f32 = 0.3;
/// Radians a second: the reference video's hero turns 180 degrees in 5 frames, see `docs/REFERENCE.md`.
const STEER_TURN_RATE: f32 = 18.8;

/// The walkmesh, as the Modern body asks it.
struct NavGround<'a>(&'a NavMesh);

impl Ground for NavGround<'_> {
    fn floor(&self, x: f32, y: f32, max_z: f32) -> Option<f32> {
        self.0.walkable_at(x, y, max_z)
    }
    fn blocked(&self, a: [f32; 3], b: [f32; 3]) -> bool {
        self.0.crosses_block(a, b)
    }
    fn wall(&self, a: [f32; 3], b: [f32; 3]) -> bool {
        self.0.sight_blocked(a, b)
    }
    fn way_back(&self, a: [f32; 3], b: [f32; 3]) -> bool {
        self.0.same_island(a, b)
    }
}

/// Ray from a camera through a pixel: `(origin, unit direction)`. The camera looks from `eye` to `target`
/// with Z up and a vertical field of view of `fov_deg`; `(px, py)` is measured from the top-left.
pub fn pixel_ray(
    eye: [f32; 3],
    target: [f32; 3],
    fov_deg: f32,
    size: (u32, u32),
    pixel: (f32, f32),
) -> ([f32; 3], [f32; 3]) {
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let norm = |a: [f32; 3]| {
        let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-9);
        [a[0] / l, a[1] / l, a[2] / l]
    };
    let fwd = norm(sub(target, eye));
    let right = norm(cross(fwd, [0.0, 0.0, 1.0]));
    let up = cross(right, fwd);
    let t = (fov_deg.to_radians() / 2.0).tan();
    let aspect = size.0 as f32 / size.1 as f32;
    let nx = (pixel.0 / size.0 as f32 * 2.0 - 1.0) * t * aspect;
    let ny = (1.0 - pixel.1 / size.1 as f32 * 2.0) * t;
    let d = [
        fwd[0] + right[0] * nx + up[0] * ny,
        fwd[1] + right[1] * nx + up[1] * ny,
        fwd[2] + right[2] * nx + up[2] * ny,
    ];
    (eye, norm(d))
}

/// Bounding sphere used for picking: centre offset above the object's origin, and radius.
fn pick_sphere(o: &Object) -> Option<([f32; 3], f32)> {
    let (up, r) = match o.kind {
        Kind::Creature if o.dead => (0.2, 0.8),
        Kind::Creature => (0.9, 0.7),
        Kind::Door => (1.2, 1.4),
        Kind::Placeable => (0.5, 0.8),
        _ => return None,
    };
    Some(([o.position[0], o.position[1], o.position[2] + up], r))
}

pub(crate) fn inside_polygon(p: [f32; 2], poly: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let mut j = poly.len().wrapping_sub(1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

impl World {
    /// Build the navigation mesh from the area layout's room walkmeshes and block closed doors.
    pub(crate) fn build_nav(&mut self, area: &str) -> Result<()> {
        let lyt = parse_lyt(&self.inst.read(area, ResType::LYT)?)?;
        let meshes: Vec<Bwm> = lyt
            .rooms
            .iter()
            .filter_map(|r| parse_bwm(&self.inst.read(&r.model, ResType::WOK).ok()?).ok())
            .collect();
        if meshes.is_empty() {
            return Err(Error::format(format!("area {area} has no walkmeshes")));
        }
        let mut nav = NavMesh::from_meshes(&meshes);
        for o in self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door && !o.open)
        {
            let (a, b) = door_line(o);
            nav.block_line(a, b);
        }
        self.nav = Some(nav);
        Ok(())
    }

    /// The nearest pickable object hit by a ray, if any (creatures, doors and placeables; not the player).
    pub fn pick(&self, origin: [f32; 3], dir: [f32; 3]) -> Option<ObjId> {
        let mut best: Option<(f32, ObjId)> = None;
        for o in self
            .objects
            .values()
            .filter(|o| o.id != self.pc && (!o.dead || self.lootable(o.id)) && o.useable)
        {
            let Some((c, r)) = pick_sphere(o) else {
                continue;
            };
            let oc = [origin[0] - c[0], origin[1] - c[1], origin[2] - c[2]];
            let b = oc[0] * dir[0] + oc[1] * dir[1] + oc[2] * dir[2];
            let cc = oc[0] * oc[0] + oc[1] * oc[1] + oc[2] * oc[2] - r * r;
            let disc = b * b - cc;
            if disc < 0.0 {
                continue;
            }
            let t = -b - disc.sqrt();
            if t >= 0.0 && best.is_none_or(|(bt, _)| t < bt) {
                best = Some((t, o.id));
            }
        }
        best.map(|(_, id)| id)
    }

    fn range_for(&self, target: ObjId) -> f32 {
        match self.objects.get(&target).map(|o| o.kind) {
            Some(Kind::Door) => DOOR_RANGE,
            Some(Kind::Creature) => TALK_RANGE,
            _ => USE_RANGE,
        }
    }

    /// Route to a place within `stop` metres of a spot that cannot be walked to, the nearest to it that can.
    fn route_beside(&mut self, to: [f32; 3], stop: f32) -> Option<Vec<[f32; 3]>> {
        const RINGS: [f32; 3] = [0.5, 0.75, 1.0];
        const DIRECTIONS: usize = 12;
        let from = self.objects.get(&self.pc)?.position;
        let nav = self.nav.as_mut()?;
        for share in RINGS {
            for d in 0..DIRECTIONS {
                let turn = d as f32 / DIRECTIONS as f32 * std::f32::consts::TAU;
                let at = [
                    to[0] + turn.cos() * stop * share,
                    to[1] + turn.sin() * stop * share,
                    to[2],
                ];
                // Nobody is spoken to through a closed door: Trask waits behind one. And nobody stands in the gap around a
                // stage to speak to its dancers: from there the next route can lead onto the stage, with no way off.
                if nav.crosses_block(at, to) || nav.height_at(at[0], at[1], at[2] + 1.0).is_none() {
                    continue;
                }
                if let Some(route) = nav.find_path(from, at) {
                    return Some(route);
                }
            }
        }
        None
    }

    /// Route from the player to within `stop` metres of `to`, or `None` when unreachable. A route shorter than `stop`
    /// is empty (already in range).
    fn route_near(
        &mut self,
        to: [f32; 3],
        stop: f32,
        lift_door: Option<ObjId>,
    ) -> Option<Vec<[f32; 3]>> {
        let from = self.objects.get(&self.pc)?.position;
        // A closed door blocks its own doorway; lift that block while planning to reach the door itself.
        let door = lift_door
            .and_then(|d| self.objects.get(&d))
            .filter(|d| !d.open)
            .map(door_line);
        let nav = self.nav.as_mut()?;
        if let Some((a, b)) = door {
            nav.unblock_line(a, b);
        }
        let route = nav.find_path(from, to);
        if let Some((a, b)) = door {
            nav.block_line(a, b);
        }
        Some(trim_route(route?, stop))
    }

    /// Click on an object: walk up to it, then talk to it, use it, or open it. Returns false when there is
    /// nothing to do (not an interactive object, or no way to reach it).
    pub fn click(&mut self, target: ObjId) -> bool {
        let Some(o) = self.objects.get(&target) else {
            return false;
        };
        if !matches!(o.kind, Kind::Creature | Kind::Door | Kind::Placeable)
            || (o.dead && !self.lootable(target))
            || target == self.pc
            || !o.useable
        {
            return false;
        }
        let (pos, kind) = (o.position, o.kind);
        let stop = self.range_for(target);
        let here = self.objects[&self.pc].position;
        if dist2d(here, pos) <= stop {
            self.pc_mover.stop();
            self.pending = None;
            self.interact(target);
            return true;
        }
        let lift = (kind == Kind::Door).then_some(target);
        let route = self.route_near(pos, stop, lift).or_else(|| {
            // Someone behind a counter stands where nobody can walk, as Janice Nall does: go to where they can be spoken to.
            // A thing with no floor under it is the same, as the Garage Head's desk is. It holds the pass to the Vulkars' lab.
            let no_floor = kind == Kind::Placeable
                && self
                    .nav
                    .as_ref()
                    .is_some_and(|n| n.height_at(pos[0], pos[1], pos[2] + 1.0).is_none());
            (kind == Kind::Creature || no_floor)
                .then(|| self.route_beside(pos, stop))
                .flatten()
        });
        let Some(route) = route else {
            self.events
                .push(format!("cannot reach {}", self.describe(target)));
            return false;
        };
        if route.len() < 2 {
            self.pending = None;
            self.interact(target);
            return true;
        }
        self.pending = Some(target);
        self.pc_mover.follow(route);
        true
    }

    /// Hand control to a party member, as the game does when the player changes character: that creature is now the one
    /// `GetFirstPC` and the mouse act on, and the old leader follows. Returns whether anything changed.
    pub fn switch_leader(&mut self, id: ObjId) -> bool {
        if id == self.pc || !self.party.contains(&id) {
            return false;
        }
        let old = self.pc;
        // The party has one bag, as the inventory screen's words say: it goes with whoever leads.
        let bag = self.carried(old);
        if let Some(o) = self.objects.get_mut(&old) {
            o.inventory.retain(|i| !bag.contains(i));
        }
        if let Some(o) = self.objects.get_mut(&id) {
            o.inventory.extend(bag);
        }
        self.combat_queue.clear();
        self.party.retain(|p| *p != id);
        self.party.push(old);
        if let Some(o) = self.objects.get_mut(&old) {
            o.party_member = true;
        }
        let (pos, facing) = match self.objects.get_mut(&id) {
            Some(o) => {
                o.party_member = false;
                (o.position, o.facing.to_radians())
            }
            None => return false,
        };
        self.pc = id;
        self.pc_mover = kotor_walkmesh::Mover::new(pos, facing);
        self.pending = None;
        self.events
            .push(format!("{} leads the party", self.describe(id)));
        true
    }

    /// Hand control to the next party member. Returns whether anything changed.
    pub fn next_leader(&mut self) -> bool {
        match self.party.first().copied() {
            Some(id) => self.switch_leader(id),
            None => false,
        }
    }

    /// Walk the player to a point on the floor (no interaction).
    pub fn walk_to(&mut self, to: [f32; 3]) -> bool {
        let from = self.objects[&self.pc].position;
        let Some(route) = self.nav.as_ref().and_then(|n| n.find_path(from, to)) else {
            return false;
        };
        self.pending = None;
        self.pc_mover.follow(route);
        true
    }

    /// What reaching an object does.
    fn interact(&mut self, target: ObjId) {
        let Some(o) = self.objects.get(&target) else {
            return;
        };
        match o.kind {
            Kind::Creature if o.dead => {
                self.last_used_by = self.pc;
                self.opened = Some(target);
            }
            Kind::Creature => {
                self.talk_to(target);
            }
            Kind::Door => self.use_door(target),
            Kind::Placeable => {
                self.last_speaker = self.pc;
                self.last_used_by = self.pc;
                let (script, conv) = (o.scripts.get("OnUsed").cloned(), o.conversation.clone());
                self.events.push(format!("used {}", self.describe(target)));
                if let Some(s) = script {
                    let _ = self.run_script(&s, target);
                }
                if self.interactive_dialogue {
                    self.open_container(target);
                }
                if !conv.is_empty() {
                    let pc = self.pc;
                    let _ = self.converse(target, pc, "", &mut crate::FirstReply);
                }
            }
            _ => {}
        }
    }

    /// Use a door: a door that leads to another module changes module; otherwise it toggles open and closed.
    pub fn use_door(&mut self, id: ObjId) {
        let Some(d) = self.objects.get(&id) else {
            return;
        };
        // A door whose neighbour is joined just opens: the player walks through, see `update_seamless`.
        if !d.link_module.is_empty() && !self.seamless.attached.contains_key(&id) {
            self.transition_request = Some((d.link_module.clone(), d.link_to.clone()));
            return;
        }
        if d.open {
            self.close_door(id);
        } else {
            if d.locked {
                self.try_lock(id);
            }
            self.open_door(id);
        }
    }

    /// The file of what has a lock: a door's or a placeable's.
    fn lock_file(&mut self, id: ObjId) -> Option<kotor_gff::Gff> {
        let o = self.objects.get(&id)?;
        let kind = if o.kind == Kind::Door { 2042 } else { 2044 };
        let template = o.template.clone();
        let bytes = self.inst.read(&template, ResType(kind)).ok()?;
        kotor_gff::Gff::parse(&bytes).ok()
    }

    /// Whether a Security check could open this: it has a DC and does not ask for a key.
    pub fn lock_can_be_picked(&mut self, id: ObjId) -> bool {
        let Some(template) = self.objects.get(&id).map(|o| o.template.clone()) else {
            return false;
        };
        if let Some(known) = self.pickable.get(&template) {
            return *known;
        }
        let can = self.lock_file(id).is_some_and(|g| {
            g.root.int("OpenLockDC").unwrap_or(0) > 0 && g.root.int("KeyRequired").unwrap_or(0) == 0
        });
        self.pickable.insert(template, can);
        can
    }

    /// What a security spike adds, from its file's property. `g_i_secspike01` has 5 and `g_i_secspike02`, the tunneler, 10.
    fn spike_bonus(&mut self, spike: ObjId) -> i32 {
        const PROPERTY_SECURITY_SPIKE: i64 = 37;
        let Some(template) = self.objects.get(&spike).map(|o| o.template.clone()) else {
            return 0;
        };
        let Some(g) = self
            .inst
            .read(&template, ResType(2025))
            .ok()
            .and_then(|b| kotor_gff::Gff::parse(&b).ok())
        else {
            return 0;
        };
        g.root
            .list("PropertiesList")
            .unwrap_or_default()
            .iter()
            .find(|p| p.int("PropertyName") == Some(PROPERTY_SECURITY_SPIKE))
            .and_then(|p| p.int("CostValue"))
            .unwrap_or(0) as i32
    }

    /// The player tries a lock: a carried key opens it, or else a Security check against its `OpenLockDC`, with the spike
    /// they chose. A lock that names `KeyRequired`, or has no DC, opens only by key or script. A spike is used up by a try.
    ///
    /// **Assumption:** a check can be tried again.
    fn try_lock(&mut self, id: ObjId) {
        let tool = self.lock_tool.take();
        let Some(utd) = self.lock_file(id) else {
            return;
        };
        let key = utd.root.str("KeyName").unwrap_or("").to_ascii_lowercase();
        let pc = self.pc;
        let has_key = !key.is_empty()
            && self.objects.get(&pc).is_some_and(|o| {
                o.inventory.iter().any(|i| {
                    self.objects
                        .get(i)
                        .is_some_and(|it| it.tag.eq_ignore_ascii_case(&key))
                })
            });
        let dc = utd.root.int("OpenLockDC").unwrap_or(0) as i32;
        let key_only = utd.root.int("KeyRequired").unwrap_or(0) != 0;
        let opened = if has_key {
            self.events
                .push(format!("{} opens with a key", self.describe(id)));
            true
        } else if dc > 0 && !key_only && self.skill_ranks(pc, SKILL_SECURITY) > 0 {
            let bonus = tool.map_or(0, |spike| self.spike_bonus(spike));
            if let Some(spike) = tool {
                self.spend_one(pc, spike);
            }
            let c = self.skill_check(pc, SKILL_SECURITY, dc - bonus);
            self.events.push(format!(
                "security {} + {} against {} on {}: {}",
                c.roll,
                c.bonus,
                dc,
                self.describe(id),
                if c.passed { "picked" } else { "failed" }
            ));
            c.passed
        } else {
            false
        };
        if opened {
            if let Some(d) = self.objects.get_mut(&id) {
                d.locked = false;
            }
        }
    }

    /// Open a door if it is not locked; the doorway becomes walkable. Returns whether it opened.
    pub fn open_door(&mut self, id: ObjId) -> bool {
        let Some(d) = self.objects.get(&id) else {
            return false;
        };
        if d.open || d.kind != Kind::Door {
            return d.open;
        }
        let locked = d.locked;
        let (on_open, on_fail) = (
            d.scripts.get("OnOpen").cloned(),
            d.scripts.get("OnFailToOpen").cloned(),
        );
        if locked {
            self.events.push(format!("{} is locked", self.describe(id)));
            if let Some(s) = on_fail {
                let _ = self.run_script(&s, id);
            }
            return false;
        }
        if let Some(d) = self.objects.get_mut(&id) {
            d.open = true;
            d.door = DoorState::Opening(0.0);
        }
        self.last_opened_by = self.pc;
        self.events.push(format!("{} opens", self.describe(id)));
        if let Some(s) = on_open {
            let _ = self.run_script(&s, id);
        }
        true
    }

    pub fn close_door(&mut self, id: ObjId) {
        let Some(d) = self.objects.get(&id) else {
            return;
        };
        if !d.open || d.kind != Kind::Door {
            return;
        }
        let (line, on_closed) = (door_line(d), d.scripts.get("OnClosed").cloned());
        // A door still opening never freed its doorway, so there is nothing to block again.
        let was_passable = d.door == DoorState::Open;
        if let Some(d) = self.objects.get_mut(&id) {
            d.open = false;
            d.door = DoorState::Closing(0.0);
        }
        if was_passable {
            if let Some(n) = self.nav.as_mut() {
                n.block_line(line.0, line.1);
            }
        }
        self.events.push(format!("{} closes", self.describe(id)));
        if let Some(s) = on_closed {
            let _ = self.run_script(&s, id);
        }
    }

    /// Fire trigger enter and exit scripts for the player and party as they cross trigger outlines.
    /// Who stands in a trigger now, nearest its first corner first.
    pub(crate) fn inside_trigger(&self, trigger: ObjId) -> std::collections::VecDeque<ObjId> {
        let Some(poly) = self.objects.get(&trigger).map(|t| &t.polygon) else {
            return Default::default();
        };
        self.objects
            .values()
            .filter(|o| o.kind == Kind::Creature && !o.dead)
            .filter(|o| inside_polygon([o.position[0], o.position[1]], poly))
            .map(|o| o.id)
            .collect()
    }

    fn update_triggers(&mut self) {
        let who: Vec<ObjId> = std::iter::once(self.pc)
            .chain(self.party.iter().copied())
            .collect();
        let triggers: Vec<ObjId> = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Trigger && o.polygon.len() >= 3)
            .map(|o| o.id)
            .collect();
        for t in triggers {
            for &w in &who {
                let (poly, pos) = match (self.objects.get(&t), self.objects.get(&w)) {
                    (Some(t), Some(w)) => (t.polygon.clone(), w.position),
                    _ => continue,
                };
                let now = inside_polygon([pos[0], pos[1]], &poly);
                let was = self.inside.contains(&(t, w));
                if now && !was {
                    self.inside.insert((t, w));
                    self.entering = w;
                    self.events
                        .push(format!("{} enters {}", self.describe(w), self.describe(t)));
                    let (script, link) = self
                        .objects
                        .get(&t)
                        .map(|o| {
                            (
                                o.scripts.get("ScriptOnEnter").cloned(),
                                (o.link_module.clone(), o.link_to.clone()),
                            )
                        })
                        .unwrap_or_default();
                    if let Some(s) = script {
                        let _ = self.run_script(&s, t);
                    }
                    self.trap_stepped_on(t, w);
                    // A neighbour joined here is walked into, see `update_seamless`.
                    if !link.0.is_empty()
                        && w == self.pc
                        && !self.seamless.attached.contains_key(&t)
                    {
                        self.transition_request = Some(link);
                    }
                } else if !now && was {
                    self.inside.remove(&(t, w));
                    self.exiting = w;
                    self.events
                        .push(format!("{} leaves {}", self.describe(w), self.describe(t)));
                    let script = self
                        .objects
                        .get(&t)
                        .and_then(|o| o.scripts.get("ScriptOnExit").cloned());
                    if let Some(s) = script {
                        let _ = self.run_script(&s, t);
                    }
                }
            }
        }
    }

    /// Move to another module: everything except the player, party and carried items is replaced; globals, journal
    /// and XP carry over. The player arrives at the waypoint tagged `waypoint` (or the module's entry point).
    pub fn transition(&mut self, module: &str, waypoint: &str) -> Result<()> {
        let mut keep: HashSet<ObjId> = std::iter::once(self.pc)
            .chain(self.party.iter().copied())
            .collect();
        let carried: Vec<ObjId> = keep
            .iter()
            .filter_map(|id| self.objects.get(id))
            .flat_map(|o| {
                o.inventory
                    .iter()
                    .copied()
                    .chain(o.equipped.values().copied())
            })
            .collect();
        keep.extend(carried);
        // The module stays as the player leaves it, for coming back.
        if !self.module.is_empty() {
            let left = self.module_snapshot(&keep);
            self.left_modules.insert(self.module.clone(), left);
        }
        self.objects.retain(|id, _| keep.contains(id));
        // What they were about to do belongs to the module they leave. Kandon's talk ended with the player due to jump to a
        // place in the Vulkar base, which is off the floor of the swoop track.
        for o in self.objects.values_mut() {
            o.actions.clear();
        }
        self.delayed.clear();
        self.inside.clear();
        self.pending = None;
        self.pc_mover.stop();
        self.load_module(module)?;
        if let Some(saved) = self.left_modules.remove(module) {
            self.restore_module(&saved, &keep);
        }
        if !waypoint.is_empty() {
            if let Some(wp) = self
                .by_tag(waypoint)
                .first()
                .and_then(|id| self.objects.get(id))
                .map(|o| o.position)
            {
                let pc = self.pc;
                if let Some(p) = self.objects.get_mut(&pc) {
                    p.position = wp;
                }
                self.pc_mover = kotor_walkmesh::Mover::new(wp, std::f32::consts::FRAC_PI_2);
                self.place_party_near(wp);
            }
        } else if let Some(entry) = self.objects.get(&self.pc).map(|o| o.position) {
            // No waypoint: the module's entry point, where `load_module` put the player. The swoop race's module is entered so.
            self.pc_mover = kotor_walkmesh::Mover::new(entry, std::f32::consts::FRAC_PI_2);
            self.place_party_near(entry);
        }
        self.events
            .push(format!("transition to {module} at {waypoint:?}"));
        self.enter_area();
        Ok(())
    }

    /// Advance the game by `dt` seconds: the clock, delayed commands and action queues, the player's walk, the
    /// interaction waiting at the end of it, trigger enter/exit, and any requested module change.
    pub fn step(&mut self, dt: f32) {
        if self.game_over {
            self.step_dead(dt);
            return;
        }
        self.tick(dt);
        self.update_minigame(dt);
        self.update_race(dt);
        self.resume_conversation();
        self.update_doors(dt);
        let before = self.pc_mover.pos;
        let modern = self.movement == MovementMode::Modern && self.nav.is_some();
        if modern {
            self.sync_body();
        }
        let roll = std::mem::take(&mut self.roll_request) && modern && self.may_roll();
        if roll {
            self.pc_mover.stop();
            self.pending = None;
            self.close_container();
        }
        // A player who is down lies where it fell while the others fight on.
        let pc_down = self.objects.get(&self.pc).is_some_and(|o| o.down || o.dead);
        if pc_down {
            self.pc_mover.stop();
            self.pending = None;
        }
        let status = match (
            self.nav.as_ref(),
            self.steer.filter(|_| self.conv.is_none() && !pc_down),
        ) {
            (Some(_), Some((dir, run))) => {
                self.pc_mover.stop();
                self.pending = None;
                self.close_container();
                if modern {
                    self.step_body(Some(dir), !run, roll, dt);
                } else {
                    self.steer_pc(dir, run, dt);
                }
                MoveStatus::Idle
            }
            (Some(_), None) if modern && self.pc_mover.has_route() && self.conv.is_none() => {
                self.step_route(dt)
            }
            // A conversation holds the walk: the panel in `end_m01ab` was reached during a talk and could not start its own.
            (Some(nav), None) if self.pc_mover.has_route() && self.conv.is_none() => {
                // The hero runs wherever the reference video was looked at, see `docs/REFERENCE.md`.
                let params = MoveParams {
                    speed: RUN_SPEED,
                    turn_rate: STEER_TURN_RATE,
                    ..self.move_params
                };
                self.pc_mover.step(nav, &params, dt)
            }
            _ => {
                if modern {
                    self.step_body(None, false, roll, dt);
                }
                MoveStatus::Idle
            }
        };
        let held_up = self.collide_pc(before, dt);
        if modern {
            self.settle_body();
        }
        let pc = self.pc;
        if let Some(p) = self.objects.get_mut(&pc) {
            p.position = self.pc_mover.pos;
        }
        self.update_followers(dt);
        self.update_combat(dt);
        self.update_stealth();
        self.update_shoves();
        self.expire_effects();
        self.update_knockouts();
        if status == MoveStatus::Arrived || held_up {
            if let Some(t) = self.pending.take() {
                self.interact(t);
            }
        }
        self.update_triggers();
        self.update_traps();
        self.update_seamless();
        self.update_encounters();
        self.update_early_force(dt);
        for item in std::mem::take(&mut self.items_acquired) {
            self.item_acquired = item;
            self.run_module_script("Mod_OnAcquirItem");
        }
        self.update_regeneration(dt);
        if let Some((module, wp)) = self.transition_request.take() {
            if let Err(e) = self.transition(&module, &wp) {
                self.errors.push(format!("transition to {module}: {e}"));
            }
        }
    }

    /// Someone else moved the player (a door, a script, a new area, a loaded game): the body starts again from there.
    fn sync_body(&mut self) {
        if self.pc_mover.pos != self.body_mark {
            self.pc_body = Body::new(self.pc_mover.pos, self.pc_mover.heading);
            self.route_speed = 0.0;
        }
        // A conversation turns the player to the speaker.
        self.pc_body.heading = self.pc_mover.heading;
    }

    /// The player may roll: on its feet, out of conversations and minigames.
    fn may_roll(&self) -> bool {
        self.conv.is_none()
            && self.minigame.is_none()
            && self.race.is_none()
            && self
                .objects
                .get(&self.pc)
                .is_some_and(|o| !o.dead && !o.down)
    }

    fn step_body(&mut self, dir: Option<[f32; 2]>, walk: bool, roll: bool, dt: f32) {
        let Some(nav) = self.nav.as_ref() else {
            return;
        };
        let input = Input { dir, walk, roll };
        let ev = self.pc_body.step(input, &NavGround(nav), &self.tuning, dt);
        self.body_cues.merge(ev);
        self.pc_mover.pos = self.pc_body.pos;
        self.pc_mover.heading = self.pc_body.heading;
    }

    /// Along a click's route: speeding up from a stand and slowing down to stop where it ends.
    fn step_route(&mut self, dt: f32) -> MoveStatus {
        let t = self.tuning;
        let brake = (2.0 * t.decel * self.pc_mover.remaining()).sqrt();
        let want = t.sprint.min(brake);
        self.route_speed = if want > self.route_speed {
            (self.route_speed + t.accel * dt).min(want)
        } else {
            want
        };
        let params = MoveParams {
            // Never so slow near the end that it creeps.
            speed: self.route_speed.max(1.0),
            turn_rate: STEER_TURN_RATE,
            ..self.move_params
        };
        let Some(nav) = self.nav.as_ref() else {
            return MoveStatus::Idle;
        };
        let status = self.pc_mover.step(nav, &params, dt);
        let p = self.pc_mover.pos;
        let ev =
            self.pc_body
                .carried_to([p[0], p[1]], self.pc_mover.heading, &NavGround(nav), &t, dt);
        self.body_cues.merge(ev);
        self.pc_mover.pos[2] = self.pc_body.pos[2];
        if status == MoveStatus::Arrived {
            self.route_speed = 0.0;
            self.pc_body.vel = [0.0; 2];
        }
        status
    }

    /// Others may have pushed the player aside: the body keeps its own height, and goes where the player is.
    fn settle_body(&mut self) {
        self.pc_body.pos[0] = self.pc_mover.pos[0];
        self.pc_body.pos[1] = self.pc_mover.pos[1];
        self.pc_mover.pos[2] = self.pc_body.pos[2];
        self.body_mark = self.pc_mover.pos;
    }

    /// How fast a party member goes to catch up. In Modern movement the player is faster than a run: the further
    /// behind they are, the closer to a little over its speed. **Assumed:** 10% over, reached 4 m past where they set off.
    pub(crate) fn follow_speed(&self, behind: f32) -> f32 {
        if self.movement == MovementMode::Classic {
            return RUN_SPEED;
        }
        let share = ((behind - crate::follow::FOLLOW_START) / 4.0).clamp(0.0, 1.0);
        RUN_SPEED + (self.tuning.sprint * 1.1 - RUN_SPEED) * share
    }

    /// Move the player along `dir` (a unit vector), sliding along walls and closed doors, and turn to face it.
    fn steer_pc(&mut self, dir: [f32; 2], run: bool, dt: f32) {
        let Some(nav) = self.nav.as_ref() else {
            return;
        };
        let p = self.pc_mover.pos;
        let go = if run {
            RUN_SPEED
        } else {
            self.move_params.speed
        } * dt;
        let open = |d: [f32; 2]| -> Option<[f32; 3]> {
            let (x, y) = (p[0] + d[0] * go, p[1] + d[1] * go);
            if nav.crosses_block(p, [x, y, p[2]]) {
                return None;
            }
            let len = d[0].hypot(d[1]).max(1e-6);
            nav.walkable_at(
                x + d[0] / len * STEER_CLEARANCE,
                y + d[1] / len * STEER_CLEARANCE,
                p[2] + 0.6,
            )?;
            Some([x, y, nav.walkable_at(x, y, p[2] + 0.6)?])
        };
        if let Some(q) = open(dir)
            .or_else(|| open([dir[0], 0.0]))
            .or_else(|| open([0.0, dir[1]]))
        {
            self.pc_mover.pos = q;
        }
        let tau = std::f32::consts::TAU;
        let h = self.pc_mover.heading;
        let err = (dir[1].atan2(dir[0]) - h + tau * 1.5).rem_euclid(tau) - tau / 2.0;
        let most = STEER_TURN_RATE * dt;
        self.pc_mover.heading = h + err.clamp(-most, most);
    }

    /// One step of the `MoveTo` at the head of an object's queue. Returns whether it is finished.
    pub(crate) fn step_move(&mut self, id: ObjId, dt: f32) -> bool {
        let Some(o) = self.objects.get(&id) else {
            return true;
        };
        let pos = o.position;
        let facing = o.facing.to_radians();
        let Some(crate::world::Action::MoveTo {
            target,
            to,
            range,
            run,
            walker,
        }) = o.actions.front().cloned()
        else {
            return true;
        };
        let mut mover = match walker {
            Some(m) => m,
            None => {
                let goal = target
                    .and_then(|t| self.objects.get(&t))
                    .map_or(to, |t| t.position);
                if dist2d(pos, goal) <= range {
                    return true;
                }
                let Some(route) = self.route_through_doors(pos, goal) else {
                    self.events
                        .push(format!("{} has no way there", self.describe(id)));
                    return true;
                };
                let route = trim_route(route, range);
                if route.len() < 2 {
                    return true;
                }
                let mut m = Mover::new(pos, facing);
                m.follow(route);
                m
            }
        };
        self.open_doors_near(pos);
        let params = MoveParams {
            speed: if run {
                RUN_SPEED
            } else {
                self.move_params.speed
            },
            ..self.move_params
        };
        let Some(nav) = self.nav.as_ref() else {
            return true;
        };
        let status = mover.step(nav, &params, dt);
        // Walkers go round the people in their way (bug 15), except the one they walk to.
        if id != self.pc && mover.pos != pos {
            let blockers: Vec<[f32; 2]> = self
                .objects
                .values()
                .filter(|o| {
                    o.kind == Kind::Creature && !o.dead && o.id != id && Some(o.id) != target
                })
                .map(|o| [o.position[0], o.position[1]])
                .collect();
            mover.pos = slide_around(nav, pos, mover.pos, &blockers);
        }
        let now = mover.pos;
        if let Some(o) = self.objects.get_mut(&id) {
            o.position = now;
            o.facing = mover.heading.to_degrees();
        }
        if id == self.pc {
            self.pc_mover.pos = now;
            self.pc_mover.heading = mover.heading;
        }
        if matches!(status, MoveStatus::Arrived | MoveStatus::Idle) {
            return true;
        }
        if let Some(crate::world::Action::MoveTo { walker, .. }) = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.actions.front_mut())
        {
            *walker = Some(mover);
        }
        false
    }

    /// A route that may pass closed doors that are not locked; `open_doors_near` opens them on the way.
    pub(crate) fn route_through_doors(
        &mut self,
        from: [f32; 3],
        to: [f32; 3],
    ) -> Option<Vec<[f32; 3]>> {
        let doors: Vec<([f32; 3], [f32; 3])> = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door && !o.open && !o.locked && self.in_the_way(o))
            .map(door_line)
            .collect();
        let nav = self.nav.as_mut()?;
        for (a, b) in &doors {
            nav.unblock_line(*a, *b);
        }
        let route = nav.find_path(from, to);
        for (a, b) in &doors {
            nav.block_line(*a, *b);
        }
        route
    }

    pub(crate) fn open_doors_near(&mut self, at: [f32; 3]) {
        let near: Vec<ObjId> = self
            .objects
            .values()
            .filter(|o| {
                o.kind == Kind::Door
                    && !o.open
                    && !o.locked
                    && o.link_module.is_empty()
                    && dist2d(o.position, at) < DOOR_RANGE
            })
            .map(|o| o.id)
            .collect();
        for d in near {
            self.open_door(d);
        }
    }

    /// The nearest thing the player can use: creature, door or placeable within `reach` metres.
    pub fn nearest_usable(&self, reach: f32) -> Option<ObjId> {
        let here = self.objects.get(&self.pc)?.position;
        self.objects
            .values()
            .filter(|o| {
                matches!(o.kind, Kind::Creature | Kind::Door | Kind::Placeable)
                    && o.id != self.pc
                    && (!o.dead || self.lootable(o.id))
                    && !o.party_member
                    && o.useable
            })
            .map(|o| (dist2d(o.position, here), o.id))
            .filter(|(d, _)| *d <= reach)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    /// Open a placeable that has an inventory: unless it is locked, its `ItemList` goes into it the first time, it is
    /// open, and its `OnOpen` script runs, which is how the game's own treasure scripts fill it.
    ///
    /// **Measured over the 117 modules:** 408 of 1,467 placeable templates have `HasInventory`, 354 have an `ItemList`
    /// and 45 are locked.
    fn open_container(&mut self, id: ObjId) {
        let Some(template) = self.objects.get(&id).map(|o| o.template.clone()) else {
            return;
        };
        let utp = self
            .inst
            .read(&template, ResType(2044))
            .ok()
            .and_then(|b| kotor_gff::Gff::parse(&b).ok());
        let has = utp
            .as_ref()
            .is_some_and(|g| g.root.int("HasInventory").is_some_and(|h| h != 0));
        let holds = self
            .objects
            .get(&id)
            .is_some_and(|o| !o.inventory.is_empty());
        if !has && !holds {
            return;
        }
        if self.objects.get(&id).is_some_and(|o| o.locked) {
            self.try_lock(id);
        }
        if self.objects.get(&id).is_some_and(|o| o.locked) {
            self.events.push(format!("{} is locked", self.describe(id)));
            self.run_event(id, "OnFailToOpen");
            return;
        }
        if self.filled.insert(id) {
            // One opened before its first heartbeat has its treasure too.
            self.run_event(id, "OnHeartbeat");
            let names: Vec<String> = utp
                .as_ref()
                .and_then(|g| g.root.list("ItemList"))
                .unwrap_or_default()
                .iter()
                .filter_map(|i| i.str("InventoryRes").map(str::to_string))
                .collect();
            for n in names {
                let item = self.spawn_item(&n);
                if let Some(o) = self.objects.get_mut(&id) {
                    o.inventory.push(item);
                }
            }
        }
        self.opened = Some(id);
        self.last_opened_by = self.pc;
        self.run_event(id, "OnOpen");
    }

    /// Close what is open: its `OnClosed` script runs.
    pub fn close_container(&mut self) {
        if let Some(id) = self.opened.take() {
            self.last_closed_by = self.pc;
            self.run_event(id, "OnClosed");
        }
    }

    /// A body that has something left on it. Those can be clicked and opened.
    pub fn lootable(&self, id: ObjId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.kind == Kind::Creature && o.dead && !o.inventory.is_empty())
    }

    /// What a creature that has just died leaves on its body: what its template marks `Dropable`, and what scripts
    /// gave it. The rest goes with it. The name is what `bodybag.2da` calls the default bag, since every creature
    /// template in the game has `BodyBag` 0 (measured over the BIFs and all 117 modules). The other rows' bags are
    /// models, and this engine keeps the items on the body.
    pub(crate) fn leave_loot(&mut self, id: ObjId) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let (carried, worn): (Vec<ObjId>, Vec<(i32, ObjId)>) = (
            o.inventory.clone(),
            o.equipped.iter().map(|(s, i)| (*s, *i)).collect(),
        );
        // **Assumption:** the remains hold everything carried, whatever the item's Droppable says (bug 34: the Jedi's
        // two lightsabers). Only a creature's own claws, bite and hide (`baseitems.2da` rows 81 to 84) go with it.
        let drops = |w: &World, i: &ObjId| {
            w.objects
                .get(i)
                .is_some_and(|o| o.droppable || !(81..=84).contains(&o.base_item))
        };
        let mut loot = Vec::new();
        for i in carried {
            if drops(self, &i) {
                loot.push(i);
            } else {
                self.objects.remove(&i);
            }
        }
        let mut stripped = Vec::new();
        for (slot, i) in worn {
            if drops(self, &i) {
                stripped.push(slot);
                loot.push(i);
            }
        }
        let remains = self
            .table("bodybag")
            .and_then(|t| t.int(0, "name"))
            .map(|s| self.strref_text(s as i32))
            .filter(|s| !s.is_empty());
        if let Some(o) = self.objects.get_mut(&id) {
            for s in stripped {
                o.equipped.remove(&s);
            }
            o.inventory = loot;
            if let (false, Some(name)) = (o.inventory.is_empty(), remains) {
                o.name = name;
            }
        }
    }

    /// The items in the open container: id and name, with the count of a stack.
    pub fn container_items(&self) -> Vec<(ObjId, String)> {
        let Some(c) = self.opened.and_then(|id| self.objects.get(&id)) else {
            return Vec::new();
        };
        c.inventory
            .iter()
            .filter_map(|i| self.objects.get(i))
            .map(|o| {
                let name = if o.name.is_empty() {
                    o.template.clone()
                } else {
                    o.name.clone()
                };
                let shown = if o.stack > 1 {
                    format!("{name} ({})", o.stack)
                } else {
                    name
                };
                (o.id, shown)
            })
            .collect()
    }

    /// Move an item from the open container to the player: its `OnInvDisturbed` script runs, which may destroy the
    /// container.
    pub fn take_item(&mut self, item: ObjId) {
        let pc = self.pc;
        let Some(from) = self.opened else { return };
        if let Some(c) = self.objects.get_mut(&from) {
            c.inventory.retain(|i| *i != item);
        }
        self.last_disturbed_by = pc;
        self.disturbed_item = item;
        self.run_event(from, "OnInvDisturbed");
        self.carry_acquired(pc, item);
        if !self.objects.contains_key(&from) {
            self.opened = None;
        }
        // `k_pend_chest02` moves the tutorial on from here.
        if let Some(c) = self.opened {
            if let Some(s) = self
                .objects
                .get(&c)
                .and_then(|o| o.scripts.get("OnInvDisturbed").cloned())
            {
                let _ = self.run_script(&s, c);
            }
        }
    }

    /// Advance door animations; a door that finished opening lets the party through.
    fn update_doors(&mut self, dt: f32) {
        let doors: Vec<ObjId> = self
            .objects
            .values()
            .filter(|o| matches!(o.door, DoorState::Opening(_) | DoorState::Closing(_)))
            .map(|o| o.id)
            .collect();
        for id in doors {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let line = door_line(o);
            match o.door {
                DoorState::Opening(t) if t + dt >= DOOR_ANIM_SECONDS => {
                    o.door = DoorState::Open;
                    if let Some(n) = self.nav.as_mut() {
                        n.unblock_line(line.0, line.1);
                    }
                }
                DoorState::Opening(t) => o.door = DoorState::Opening(t + dt),
                DoorState::Closing(t) if t + dt >= DOOR_ANIM_SECONDS => o.door = DoorState::Closed,
                DoorState::Closing(t) => o.door = DoorState::Closing(t + dt),
                _ => {}
            }
        }
    }

    /// Whether the player is walking right now.
    pub fn pc_walking(&self) -> bool {
        self.pc_mover.has_route()
    }

    pub fn invalid() -> ObjId {
        OBJECT_INVALID
    }
}
