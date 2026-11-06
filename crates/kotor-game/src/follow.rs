//! Party following and creature collision.
//!
//! **Assumptions, not measured from the game:** a creature occupies a disc of 0.4 m radius; a party member starts
//! walking to the player when more than 3.5 m away, stops within 2.0 m, and replans about once a second.

use crate::world::{Kind, ObjId, World};
use kotor_walkmesh::{Mover, NavMesh};
use std::collections::BTreeMap;

/// How long the player presses on people, getting no nearer, before it walks through them.
const PRESSED_SECONDS: f32 = 0.5;
/// Radius of the disc a creature occupies (metres).
pub const CREATURE_RADIUS: f32 = 0.4;
/// A party member this far from the player starts walking towards it.
pub const FOLLOW_START: f32 = 3.5;
/// A party member this close to the player stops.
pub const FOLLOW_STOP: f32 = 2.0;
/// Seconds between route replans for a following party member.
pub const REPLAN_SECONDS: f32 = 1.0;

pub(crate) fn dist2d(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Cut `stop` metres off the end of a route; a route shorter than that becomes empty.
pub(crate) fn trim_route(mut route: Vec<[f32; 3]>, stop: f32) -> Vec<[f32; 3]> {
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
            return route;
        }
        left -= seg;
        route.pop();
    }
    Vec::new()
}

/// Move from `old` to `new` without ending inside any blocker disc. A mover that would end inside one is put on
/// its edge and slides along it by the length of its step (towards the side it was already drifting to, else
/// counter-clockwise), so it goes around instead of stopping. If the slide leaves the floor it stays at `old`.
pub(crate) fn slide_around(
    nav: &NavMesh,
    old: [f32; 3],
    new: [f32; 3],
    blockers: &[[f32; 2]],
) -> [f32; 3] {
    let reach = 2.0 * CREATURE_RADIUS;
    let mut at = new;
    for _ in 0..2 {
        for c in blockers {
            let (dx, dy) = (at[0] - c[0], at[1] - c[1]);
            let dist = dx.hypot(dy);
            if dist >= reach {
                continue;
            }
            let (nx, ny) = if dist > 1e-4 {
                (dx / dist, dy / dist)
            } else {
                let (ox, oy) = (old[0] - c[0], old[1] - c[1]);
                let l = ox.hypot(oy).max(1e-4);
                (ox / l, oy / l)
            };
            let (mx, my) = (new[0] - old[0], new[1] - old[1]);
            let step = mx.hypot(my);
            let (tx, ty) = (-ny, nx);
            let side = if mx * tx + my * ty < 0.0 { -1.0 } else { 1.0 };
            let (px, py) = (nx * reach + tx * side * step, ny * reach + ty * side * step);
            let l = px.hypot(py).max(1e-4);
            let (x, y) = (c[0] + px / l * reach, c[1] + py / l * reach);
            match nav.height_at(x, y, at[2] + 0.4) {
                Some(z) => at = [x, y, z],
                None => return old,
            }
        }
    }
    at
}

impl World {
    /// Centres of living creatures that block `who`: everyone but itself, and the party too unless `who` is the
    /// player (the player pushes through its own party).
    fn blockers_for(&self, who: ObjId) -> Vec<[f32; 2]> {
        self.objects
            .values()
            .filter(|o| o.kind == Kind::Creature && !o.dead && o.id != who)
            .filter(|o| who != self.pc || !(o.party_member || self.party.contains(&o.id)))
            .map(|o| [o.position[0], o.position[1]])
            .collect()
    }

    /// Keep the player out of other creatures after it moved from `old`. Returns whether it has gone as far as it can,
    /// someone standing where its route ends. When going round people gets it no nearer for a moment, it walks through:
    /// someone on a corner of the route, beside a wall, or two people with no gap between them would hold it for good.
    pub(crate) fn collide_pc(&mut self, old: [f32; 3], dt: f32) -> bool {
        let pos = self.pc_mover.pos;
        if pos == old {
            return false;
        }
        let blockers = self.blockers_for(self.pc);
        let reach = 2.0 * CREATURE_RADIUS;
        let on = |c: &[f32; 2], p: [f32; 3]| (p[0] - c[0]).hypot(p[1] - c[1]) < reach;
        let next = self.pc_mover.next_waypoint();
        if let Some(next) = next.filter(|_| self.pc_mover.on_last_leg()) {
            // Who stands there is often who the player was walking up to, as in a fight.
            if blockers.iter().any(|c| on(c, next)) && dist2d(pos, next) < 2.0 * reach {
                self.pc_mover.stop();
                self.pc_mover.pos = old;
                self.pc_pressed = 0.0;
                return true;
            }
        }
        if self.pc_pressed >= PRESSED_SECONDS {
            if !blockers.iter().any(|c| on(c, pos)) {
                self.pc_pressed = 0.0;
            }
            return false;
        }
        let Some(nav) = self.nav.as_ref() else {
            return false;
        };
        let slid = slide_around(nav, old, pos, &blockers);
        self.pc_mover.pos = slid;
        let nearer = next.is_some_and(|n| dist2d(slid, n) < dist2d(old, n) - 1e-3);
        self.pc_pressed = if nearer { 0.0 } else { self.pc_pressed + dt };
        false
    }

    /// Walk each party member towards the player when it has fallen behind, keeping it out of other creatures.
    pub(crate) fn update_followers(&mut self, dt: f32) {
        if self.nav.is_none() {
            return;
        }
        let Some(pc_pos) = self.objects.get(&self.pc).map(|o| o.position) else {
            return;
        };
        self.replan_in -= dt;
        let replan = self.replan_in <= 0.0;
        if replan {
            self.replan_in = REPLAN_SECONDS;
        }
        let mut movers: BTreeMap<ObjId, Mover> = std::mem::take(&mut self.followers);
        movers.retain(|id, _| self.party.contains(id));
        for id in self.party.clone() {
            let Some(o) = self.objects.get(&id).filter(|o| !o.dead) else {
                continue;
            };
            let old = o.position;
            let m = movers.entry(id).or_insert_with(|| Mover::new(old, 0.0));
            m.pos = old;
            let d = dist2d(old, pc_pos);
            let nav = self.nav.as_ref().expect("checked above");
            if d <= FOLLOW_STOP {
                m.stop();
            } else if d > FOLLOW_START && (replan || !m.has_route()) {
                if let Some(route) = nav.find_path(old, pc_pos) {
                    m.follow(trim_route(route, FOLLOW_STOP));
                }
            }
            if m.has_route() {
                // The party keeps up with a running player.
                let params = kotor_walkmesh::MoveParams {
                    speed: crate::RUN_SPEED,
                    ..self.move_params
                };
                m.step(nav, &params, dt);
            }
            if m.pos != old {
                let blockers = self.blockers_for(id);
                m.pos = slide_around(nav, old, m.pos, &blockers);
            }
            let to = m.pos;
            if let Some(o) = self.objects.get_mut(&id) {
                o.position = to;
            }
        }
        self.followers = movers;
    }

    /// Put party members near `at` on arrival in an area: the first free spots around it that are on the floor.
    pub(crate) fn place_party_near(&mut self, at: [f32; 3]) {
        self.followers.clear();
        let offsets = [
            (-1.2, 0.0),
            (0.0, -1.2),
            (0.0, 1.2),
            (1.2, 0.0),
            (-1.2, -1.2),
            (-1.2, 1.2),
        ];
        let mut taken: Vec<[f32; 2]> = vec![[at[0], at[1]]];
        for id in self.party.clone() {
            let Some(nav) = self.nav.as_ref() else {
                return;
            };
            let spot = offsets.iter().find_map(|(dx, dy)| {
                let (x, y) = (at[0] + dx, at[1] + dy);
                let z = nav.height_at(x, y, at[2] + 0.4)?;
                taken
                    .iter()
                    .all(|t| (t[0] - x).hypot(t[1] - y) >= 2.0 * CREATURE_RADIUS)
                    .then_some([x, y, z])
            });
            let to = spot.unwrap_or(at);
            taken.push([to[0], to[1]]);
            if let Some(o) = self.objects.get_mut(&id) {
                o.position = to;
            }
        }
    }
}
