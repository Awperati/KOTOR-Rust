//! What the stage shows of a lift ride (`kotor_game::LiftRide`): the screen goes dark as the doors close, the car hums and
//! shakes, the module changes in the dark, and the screen comes back on the other floor. The cheap ride; `docs/LIFTS.md`.
//!
//! **Assumed:** the install has no lift sound, so the hum is the rumble of `as_ot_rumble_0*`, one every 1.8 s; the shake is two
//! small sines on the camera, a centimetre or so.
use super::Stage;
use crate::LiftView;
use kotor_game::LiftWay;
use kotor_image::Image;

/// Seconds of the ride that go to the screen darkening, as a share of it.
const FADE_OUT_SHARE: f32 = 0.15;
/// Seconds the screen takes to come back after arriving.
const FADE_IN_SECONDS: f32 = 0.9;
/// Seconds between one hum and the next.
const HUM_EVERY: f32 = 1.8;
/// Metres a ride rises or falls in a second. Assumed: the install does not say how far apart the levels are.
const METRES_A_SECOND: f32 = 1.5;
/// How far above the car the arriving picture is cut off at the end, in metres: higher than anything in view.
const REACH: f32 = 40.0;

#[derive(Default)]
pub(crate) struct LiftStage {
    /// A ride was going on at the last frame.
    was: bool,
    /// Seconds since the ride began.
    clock: f32,
    /// Seconds since the last hum, and how many there were.
    hum: f32,
    hums: usize,
    /// Seconds of fade-in left after arriving.
    after: f32,
    /// The picture of the arrival is up, with its slide: how far, which way.
    placed: Option<(f32, LiftWay)>,
    /// Where the car stands, in the module being left.
    car: [f32; 3],
    /// The HUD with the dark laid over it, while there is any.
    pub(crate) dark: Option<Image>,
}

impl Stage {
    pub(super) fn lift_tick(&mut self, dt: f32) {
        let riding = self.game.world.lift_ride().is_some();
        if riding {
            if !self.lift.was {
                self.lift.clock = 0.0;
                self.lift.hum = HUM_EVERY;
                self.lift.hums = 0;
                self.lift_place();
            }
            self.lift.clock += dt;
            self.lift.hum += dt;
            if self.lift.hum >= HUM_EVERY {
                self.lift.hum = 0.0;
                let n = self.lift.hums % 5 + 1;
                self.lift.hums += 1;
                self.effect(&format!("as_ot_rumble_0{n}"));
            }
            self.lift.after = 0.0;
        } else if self.lift.was {
            // Arrived: the screen has been dark, and comes back.
            self.lift.after = if self.lift.placed.is_some() {
                0.0
            } else {
                FADE_IN_SECONDS
            };
        } else if self.lift.after > 0.0 {
            self.lift.after = (self.lift.after - dt).max(0.0);
        }
        self.lift.was = riding;
        if !riding && self.lift.placed.take().is_some() {
            self.lift_set_arrival(None);
        }
    }

    /// Put the module being arrived in below or above this one, its arrival waypoint where the car is and facing as the car
    /// does, when it is loaded ahead and the ride rises or falls. Else the ride stays the cheap one.
    fn lift_place(&mut self) {
        let Some(ride) = self.game.world.lift_ride().cloned() else {
            return;
        };
        if ride.way == LiftWay::Level {
            return;
        }
        let Some((scene, wp, wp_face)) = self.lift_arrival(&ride.to, &ride.waypoint) else {
            return;
        };
        let pc = self.game.world.pc;
        let Some(me) = self.game.world.object(pc) else {
            return;
        };
        let (car, face) = (me.position, me.facing.to_radians());
        let height = ride.seconds * METRES_A_SECOND;
        let yaw = face - wp_face;
        let (sn, cs) = yaw.sin_cos();
        let shift = [
            car[0] - (cs * wp[0] - sn * wp[1]),
            car[1] - (sn * wp[0] + cs * wp[1]),
            car[2] - wp[2]
                + if ride.way == LiftWay::Down {
                    -height
                } else {
                    height
                },
        ];
        self.lift.car = car;
        self.lift.placed = Some((height, ride.way));
        self.lift_set_arrival(Some(scene.moved(yaw, shift)));
    }

    /// How the ride looks to the renderer this frame: the pictures' slide and where they meet. Nothing when there is none.
    pub fn lift_view(&self) -> Option<LiftView> {
        let ride = self.game.world.lift_ride()?;
        let (height, way) = self.lift.placed?;
        let t = ride.progress();
        let slide = t * t * (3.0 - 2.0 * t);
        let (up, arriving_above, boundary) = match way {
            LiftWay::Down => (
                height * slide,
                false,
                self.lift.car[2] - 0.3 + REACH * slide,
            ),
            _ => (
                -height * slide,
                true,
                self.lift.car[2] + 3.0 + REACH * (1.0 - slide),
            ),
        };
        Some(LiftView {
            left_up: up,
            arriving_up: up,
            boundary,
            arriving_above,
        })
    }

    /// How dark the screen is, 0 to 1.
    pub fn lift_darkness(&self) -> f32 {
        if self.lift.placed.is_some() {
            return 0.0;
        }
        if let Some(r) = self.game.world.lift_ride() {
            return (r.progress() / FADE_OUT_SHARE).min(1.0);
        }
        if self.lift.was {
            // The ride ended this frame and the module changes now: still black.
            return 1.0;
        }
        (self.lift.after / FADE_IN_SECONDS).clamp(0.0, 1.0)
    }

    /// The shake of the camera, metres up and down.
    pub fn lift_camera_shake(&self) -> f32 {
        if self.game.world.lift_ride().is_none() {
            return 0.0;
        }
        let t = self.lift.clock;
        (t * 38.0).sin() * 0.012 + (t * 17.0).sin() * 0.01
    }

    /// Lay the dark over the HUD's picture, which is the one thing drawn over the scene.
    pub(super) fn lift_fade(&mut self) {
        let dark = self.lift_darkness();
        if dark <= 0.0 {
            if self.lift.dark.take().is_some() {
                self.overlay_version += 1;
            }
            return;
        }
        let Some(base) = self.overlay.as_ref() else {
            return;
        };
        let keep = 1.0 - dark;
        let mut img = base.clone();
        for p in img.rgba.chunks_mut(4) {
            p[0] = (p[0] as f32 * keep) as u8;
            p[1] = (p[1] as f32 * keep) as u8;
            p[2] = (p[2] as f32 * keep) as u8;
            p[3] = 255 - ((255 - p[3]) as f32 * keep) as u8;
        }
        self.lift.dark = Some(img);
        self.overlay_version += 1;
    }
}
