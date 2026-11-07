//! A thrown grenade is in the air before it bursts (the owner's bug of 7 Nov 2026: it burst the moment it was thrown, in
//! the Endar Spire's opening and in a fight alike). Both ways of throwing one, the player's use of the item and a script's
//! `ActionUseTalentOnObject`, end in `cast_spell`, which now starts a flight instead of running the power's script. The
//! script runs where the grenade lands.
//!
//! **Measured:** the hand lets go at 0.83 s of the 1.5 s `throwgren` clip (the right hand's fastest moment, the
//! `throw_release` example of `kotor-model`). **Assumed:** the grenade flies at 10 m/s on an arc a seventh of its length
//! high, bursts on landing with no fuse wait, and lands where its target stood when it was let go.

use crate::world::{ObjId, World};

/// When in the throw the grenade leaves the hand.
pub const RELEASE_SECONDS: f32 = 0.83;
/// How fast it flies, in metres a second.
pub const THROW_SPEED: f32 = 10.0;
/// The shortest flight, so that a grenade at someone's feet is still seen to drop.
const LEAST_FLIGHT: f32 = 0.3;
/// Where a grenade is let go, above the thrower's feet; and where it lands, above the target's.
const HAND_HEIGHT: f32 = 1.6;
const LANDING_HEIGHT: f32 = 0.2;
/// The grenade of a script's throw, which names no item.
pub(crate) const DEFAULT_GRENADE: &str = "g_w_fraggren01";

/// A grenade on its way.
#[derive(Clone, Debug)]
pub struct Flight {
    pub caster: ObjId,
    pub target: ObjId,
    pub spell: i32,
    /// The item's template, for drawing it.
    pub template: String,
    pub release_at: f32,
    pub released: bool,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub left_hand_at: f32,
    pub land_at: f32,
}

/// What a screen needs to draw one grenade in the air.
#[derive(Clone, Debug, PartialEq)]
pub struct InFlight {
    pub position: [f32; 3],
    pub template: String,
}

impl World {
    /// Whether a power is a throw: `spells.2da` gives it the `throw` gesture, as grenades have.
    pub(crate) fn is_throw(&mut self, spell: i32) -> bool {
        let t = self.table("spells");
        let get = |c: &str| {
            t.as_ref()
                .and_then(|t| t.get(spell.max(0) as usize, c))
                .unwrap_or("")
                .to_ascii_lowercase()
        };
        get("conjanim") == "throw" || get("castanim") == "throw"
    }

    /// The grenade is thrown: it leaves the hand at `RELEASE_SECONDS` and its power happens where it lands.
    pub(crate) fn start_flight(&mut self, caster: ObjId, spell: i32, target: ObjId) {
        let template = self
            .thrown_item
            .take()
            .unwrap_or_else(|| DEFAULT_GRENADE.to_string());
        self.flights.push(Flight {
            caster,
            target,
            spell,
            template,
            release_at: self.time + RELEASE_SECONDS,
            released: false,
            from: [0.0; 3],
            to: [0.0; 3],
            left_hand_at: 0.0,
            land_at: f32::INFINITY,
        });
    }

    /// Let go of the grenades whose time has come, and burst those that have landed.
    pub(crate) fn update_flights(&mut self) {
        if self.flights.is_empty() {
            return;
        }
        let now = self.time;
        for i in 0..self.flights.len() {
            let f = self.flights[i].clone();
            if !f.released && now >= f.release_at {
                let at = |w: &World, id: ObjId, up: f32| {
                    w.objects
                        .get(&id)
                        .map(|o| [o.position[0], o.position[1], o.position[2] + up])
                };
                let from = at(self, f.caster, HAND_HEIGHT).unwrap_or([0.0; 3]);
                let to = at(self, f.target, LANDING_HEIGHT).unwrap_or(from);
                let length = (to[0] - from[0]).hypot(to[1] - from[1]);
                let g = &mut self.flights[i];
                g.released = true;
                (g.from, g.to) = (from, to);
                g.left_hand_at = now;
                g.land_at = now + (length / THROW_SPEED).max(LEAST_FLIGHT);
            }
        }
        let landed: Vec<Flight> = self
            .flights
            .iter()
            .filter(|f| f.released && now >= f.land_at)
            .cloned()
            .collect();
        self.flights.retain(|f| !(f.released && now >= f.land_at));
        for f in landed {
            self.burst(f.caster, f.spell, f.target);
        }
    }

    /// Where a grenade is at `now`: along its arc, between the hand and the place it lands.
    pub fn flight_position(f: &Flight, now: f32) -> Option<[f32; 3]> {
        if !f.released || now >= f.land_at {
            return None;
        }
        let k = ((now - f.left_hand_at) / (f.land_at - f.left_hand_at)).clamp(0.0, 1.0);
        let length = (f.to[0] - f.from[0]).hypot(f.to[1] - f.from[1]);
        let lift = 4.0 * (length / 7.0) * k * (1.0 - k);
        let at = |a: f32, b: f32| a + (b - a) * k;
        Some([
            at(f.from[0], f.to[0]),
            at(f.from[1], f.to[1]),
            at(f.from[2], f.to[2]) + lift,
        ])
    }

    /// The grenades in the air, for a screen.
    pub fn grenades_in_flight(&self) -> Vec<InFlight> {
        self.flights
            .iter()
            .filter_map(|f| {
                Some(InFlight {
                    position: World::flight_position(f, self.time)?,
                    template: f.template.clone(),
                })
            })
            .collect()
    }

    /// How many are still in the hand or in the air.
    pub fn flights_pending(&self) -> usize {
        self.flights.len()
    }
}
