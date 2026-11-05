//! Interaction and movement on top of the world: picking, click-to-walk, doors, triggers, module
//! transitions and the per-step update.

use crate::world::{Kind, ObjId, Object, World};
use kotor_core::{Error, ResType, Result};
use kotor_script::OBJECT_INVALID;
use kotor_walkmesh::{parse_bwm, parse_lyt, Bwm, MoveStatus, NavMesh};
use std::collections::HashSet;

/// Radius of the navigation block a closed door puts around itself (metres). A doorway is about two metres wide.
pub const DOOR_BLOCK_RADIUS: f32 = 1.1;
/// How close the player must get before using a door, talking to a creature, or using a placeable.
pub const DOOR_RANGE: f32 = 1.6;
pub const TALK_RANGE: f32 = 1.8;
pub const USE_RANGE: f32 = 1.6;

fn dist2d(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
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
        Kind::Creature => (0.9, 0.7),
        Kind::Door => (1.2, 1.4),
        Kind::Placeable => (0.5, 0.8),
        _ => return None,
    };
    Some(([o.position[0], o.position[1], o.position[2] + up], r))
}

fn inside_polygon(p: [f32; 2], poly: &[[f32; 2]]) -> bool {
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
            nav.block_disc(o.position, DOOR_BLOCK_RADIUS);
        }
        self.nav = Some(nav);
        Ok(())
    }

    /// The nearest pickable object hit by a ray, if any (creatures, doors and placeables; not the player).
    pub fn pick(&self, origin: [f32; 3], dir: [f32; 3]) -> Option<ObjId> {
        let mut best: Option<(f32, ObjId)> = None;
        for o in self.objects.values().filter(|o| o.id != self.pc && !o.dead) {
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
        let door_pos = lift_door
            .and_then(|d| self.objects.get(&d))
            .filter(|d| !d.open)
            .map(|d| d.position);
        let nav = self.nav.as_mut()?;
        if let Some(p) = door_pos {
            nav.unblock_disc(p, DOOR_BLOCK_RADIUS);
        }
        let route = nav.find_path(from, to);
        if let Some(p) = door_pos {
            nav.block_disc(p, DOOR_BLOCK_RADIUS);
        }
        let mut route = route?;
        // Cut `stop` metres off the end.
        let mut left = stop;
        while route.len() >= 2 {
            let (a, b) = (route[route.len() - 2], route[route.len() - 1]);
            let seg = dist2d(a, b);
            if seg > left {
                let t = (seg - left) / seg;
                let last = route.len() - 1;
                route[last] = [
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    a[2] + (b[2] - a[2]) * t,
                ];
                return Some(route);
            }
            left -= seg;
            route.pop();
        }
        Some(Vec::new())
    }

    /// Click on an object: walk up to it, then talk to it, use it, or open it. Returns false when there is
    /// nothing to do (not an interactive object, or no way to reach it).
    pub fn click(&mut self, target: ObjId) -> bool {
        let Some(o) = self.objects.get(&target) else {
            return false;
        };
        if !matches!(o.kind, Kind::Creature | Kind::Door | Kind::Placeable)
            || o.dead
            || target == self.pc
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
        let Some(route) = self.route_near(pos, stop, lift) else {
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
            Kind::Creature => {
                self.talk_to(target);
            }
            Kind::Door => self.use_door(target),
            Kind::Placeable => {
                self.last_speaker = self.pc;
                let (script, conv) = (o.scripts.get("OnUsed").cloned(), o.conversation.clone());
                self.events.push(format!("used {}", self.describe(target)));
                if let Some(s) = script {
                    let _ = self.run_script(&s, target);
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
        if !d.link_module.is_empty() {
            self.transition_request = Some((d.link_module.clone(), d.link_to.clone()));
            return;
        }
        if d.open {
            self.close_door(id);
        } else {
            self.open_door(id);
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
        let (pos, locked) = (d.position, d.locked);
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
        }
        if let Some(n) = self.nav.as_mut() {
            n.unblock_disc(pos, DOOR_BLOCK_RADIUS);
        }
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
        let (pos, on_closed) = (d.position, d.scripts.get("OnClosed").cloned());
        if let Some(d) = self.objects.get_mut(&id) {
            d.open = false;
        }
        if let Some(n) = self.nav.as_mut() {
            n.block_disc(pos, DOOR_BLOCK_RADIUS);
        }
        self.events.push(format!("{} closes", self.describe(id)));
        if let Some(s) = on_closed {
            let _ = self.run_script(&s, id);
        }
    }

    /// Fire trigger enter and exit scripts for the player and party as they cross trigger outlines.
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
                    if !link.0.is_empty() && w == self.pc {
                        self.transition_request = Some(link);
                    }
                } else if !now && was {
                    self.inside.remove(&(t, w));
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
        self.objects.retain(|id, _| keep.contains(id));
        self.delayed.clear();
        self.inside.clear();
        self.pending = None;
        self.pc_mover.stop();
        self.load_module(module)?;
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
            }
        }
        self.events
            .push(format!("transition to {module} at {waypoint:?}"));
        self.enter_area();
        Ok(())
    }

    /// Advance the game by `dt` seconds: the clock, delayed commands and action queues, the player's walk, the
    /// interaction waiting at the end of it, trigger enter/exit, and any requested module change.
    pub fn step(&mut self, dt: f32) {
        self.tick(dt);
        let status = match self.nav.as_ref() {
            Some(nav) if self.pc_mover.has_route() => {
                self.pc_mover.step(nav, &self.move_params, dt)
            }
            _ => MoveStatus::Idle,
        };
        let pc = self.pc;
        if let Some(p) = self.objects.get_mut(&pc) {
            p.position = self.pc_mover.pos;
        }
        if status == MoveStatus::Arrived {
            if let Some(t) = self.pending.take() {
                self.interact(t);
            }
        }
        self.update_triggers();
        if let Some((module, wp)) = self.transition_request.take() {
            if let Err(e) = self.transition(&module, &wp) {
                self.errors.push(format!("transition to {module}: {e}"));
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
