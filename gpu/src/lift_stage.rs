//! What the stage shows of a lift ride (`kotor_game::LiftRide`): the screen goes dark as the doors close, the car hums and
//! shakes, the module changes in the dark, and the screen comes back on the other floor. The cheap ride; `docs/LIFTS.md`.
//!
//! **Assumed:** the install has no lift sound, so the hum is the rumble of `as_ot_rumble_0*`, one every 1.8 s; the shake is two
//! small sines on the camera, a centimetre or so.
use super::Stage;
use kotor_image::Image;

/// Seconds of the ride that go to the screen darkening, as a share of it.
const FADE_OUT_SHARE: f32 = 0.15;
/// Seconds the screen takes to come back after arriving.
const FADE_IN_SECONDS: f32 = 0.9;
/// Seconds between one hum and the next.
const HUM_EVERY: f32 = 1.8;

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
            self.lift.after = FADE_IN_SECONDS;
        } else if self.lift.after > 0.0 {
            self.lift.after = (self.lift.after - dt).max(0.0);
        }
        self.lift.was = riding;
    }

    /// How dark the screen is, 0 to 1.
    pub fn lift_darkness(&self) -> f32 {
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
