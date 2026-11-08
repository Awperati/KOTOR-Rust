//! Lift rides. A door whose tag or template says elevator or lift, and that leads to another module, does not change module at
//! once: the party stands in the lift for a ride of some seconds and then arrives, so that the lower cities are felt to be lower.
//! The scripts and the arrival are the door's own; only the time between is new.
//!
//! **Assumed (the install has no lift car to animate, and does not say which module lies lower):** which way a lift goes comes
//! from the hand table `docs/lifts.tsv`; a lift with no row is level and takes `DEFAULT_SECONDS`. The ride is a held moment (the
//! player cannot steer or click); what the stage shows during it is the stage's.

use crate::world::{ObjId, World};

const TABLE_TEXT: &str = include_str!("../../../docs/lifts.tsv");
/// A lift that the table does not name.
pub const DEFAULT_SECONDS: f32 = 4.0;

/// Which way a ride goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiftWay {
    Down,
    Up,
    Level,
}

/// A ride that is going on: `World::lift_ride`.
#[derive(Clone, Debug, PartialEq)]
pub struct LiftRide {
    pub door: ObjId,
    /// Where it ends: the module and the waypoint of the door's link.
    pub to: String,
    pub waypoint: String,
    pub way: LiftWay,
    pub elapsed: f32,
    pub seconds: f32,
    /// Not a lift: a door's short dip to black while the module behind it is put up (`World::door_dip`).
    pub dip: bool,
}

impl LiftRide {
    /// How far along, 0 to 1.
    pub fn progress(&self) -> f32 {
        (self.elapsed / self.seconds.max(0.01)).clamp(0.0, 1.0)
    }
}

/// The way and the seconds of a ride from `from` to `to`.
pub fn ride_between(from: &str, to: &str) -> (LiftWay, f32) {
    for line in TABLE_TEXT.lines() {
        let line = line.trim_end_matches('\r');
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 4 {
            continue;
        }
        let seconds: f32 = c[3].trim().parse().unwrap_or(DEFAULT_SECONDS);
        let way = match c[2].trim() {
            "down" => LiftWay::Down,
            "up" => LiftWay::Up,
            _ => LiftWay::Level,
        };
        if c[0].eq_ignore_ascii_case(from) && c[1].eq_ignore_ascii_case(to) {
            return (way, seconds);
        }
        if c[0].eq_ignore_ascii_case(to) && c[1].eq_ignore_ascii_case(from) {
            let back = match way {
                LiftWay::Down => LiftWay::Up,
                LiftWay::Up => LiftWay::Down,
                LiftWay::Level => LiftWay::Level,
            };
            return (back, seconds);
        }
    }
    (LiftWay::Level, DEFAULT_SECONDS)
}

impl World {
    /// Whether `door` is a lift's door that leads to another module.
    pub fn is_lift_door(&self, door: ObjId) -> bool {
        self.objects.get(&door).is_some_and(|o| {
            !o.link_module.is_empty()
                && [o.tag.as_str(), o.template.as_str()].iter().any(|s| {
                    let s = s.to_ascii_lowercase();
                    s.contains("elev") || s.contains("lift")
                })
        })
    }

    /// Start the ride of a lift door. False when it is no lift's, or one is going on already.
    pub(crate) fn begin_lift_ride(&mut self, door: ObjId) -> bool {
        if self.lift_ride.is_some() || !self.is_lift_door(door) {
            return false;
        }
        let Some(o) = self.objects.get(&door) else {
            return false;
        };
        let (to, waypoint) = (o.link_module.clone(), o.link_to.clone());
        let (way, seconds) = ride_between(&self.module, &to);
        self.events.push(format!("the lift starts ({way:?})"));
        self.start_ride(door, to, waypoint, way, seconds, false);
        true
    }

    /// A door to a module that is not joined: a short dip to black over `door_dip` seconds, as the module behind it is put up,
    /// so that no frame is seen to freeze. False when dips are off (the default), or a ride is going on.
    pub(crate) fn begin_door_dip(&mut self, door: ObjId) -> bool {
        if self.door_dip <= 0.0 || self.lift_ride.is_some() {
            return false;
        }
        let Some(o) = self.objects.get(&door) else {
            return false;
        };
        if o.link_module.is_empty() || o.link_module.eq_ignore_ascii_case(&self.module) {
            return false;
        }
        let (to, waypoint) = (o.link_module.clone(), o.link_to.clone());
        let seconds = self.door_dip;
        self.start_ride(door, to, waypoint, LiftWay::Level, seconds, true);
        true
    }

    fn start_ride(
        &mut self,
        door: ObjId,
        to: String,
        waypoint: String,
        way: LiftWay,
        seconds: f32,
        dip: bool,
    ) {
        self.pc_mover.stop();
        self.pending = None;
        self.lift_ride = Some(LiftRide {
            door,
            to,
            waypoint,
            way,
            elapsed: 0.0,
            seconds,
            dip,
        });
    }

    /// Time goes on in the lift; at the end of the ride the module changes, as the door would have changed it.
    pub(crate) fn update_lift(&mut self, dt: f32) {
        let Some(ride) = self.lift_ride.as_mut() else {
            return;
        };
        ride.elapsed += dt;
        if ride.elapsed < ride.seconds {
            return;
        }
        if let Some(r) = self.lift_ride.take() {
            if !r.dip {
                self.events.push("the lift arrives".to_string());
            }
            self.transition_request = Some((r.to, r.waypoint));
        }
    }

    /// The ride that is going on, if one is.
    pub fn lift_ride(&self) -> Option<&LiftRide> {
        self.lift_ride.as_ref()
    }
}
