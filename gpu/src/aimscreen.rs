//! Modern combat's aim: the right button held in a fight, with an enemy in sight, or with Attack mode on. The camera comes in
//! over the right shoulder, the view narrows, the body turns to the aim, and a crosshair shows. See `docs/MODERN_COMBAT.md`.
//!
//! **Assumed, all of it:** the sizes below. The game has no such thing to measure. It also has no drawn weapon to go by, so a
//! fight, a seen enemy or Attack mode stand for one.

use super::Stage;
use kotor_game::ObjId;
use kotor_ui::Crosshair;

/// How fast the camera goes in and out, a second.
pub(super) const AIM_RATE: f32 = 12.0;
/// How far right of the player the camera stands, in metres.
pub(super) const AIM_SHOULDER: f32 = 0.55;
/// How far back it stands, in metres.
const AIM_ARM: f32 = 2.0;
/// How high it looks, over the feet, in metres.
const AIM_LOOKS_AT: f32 = 1.5;
/// How much narrower the view is, in degrees.
const AIM_NARROWER: f32 = 10.0;
/// How far down the mouse may look, and up, while aiming. Negative `pitch` puts the camera under the player's head.
pub(super) const AIM_PITCH: (f32, f32) = (-0.55, 0.9);
/// How much slower the mouse turns while aiming.
pub(super) const AIM_MOUSE: f32 = 0.6;
/// The lowest `pitch` when not aiming, as `Stage::look` has it, and how fast the camera comes back to it.
const PLAIN_LOWEST: f32 = -0.2;
const PITCH_BACK_RATE: f32 = 6.0;
const LAYOUT_HIGH: i32 = 480;
/// The least time between aimed shots, in seconds. `kotor_game::fire_times` has the clips' shots about this far apart.
const SHOT_SECONDS: f32 = kotor_game::AIMED_SHOT_SECONDS;
/// Where a bolt goes from over the player's middle, and how far short of where the ray ends, in metres.
const BOLT_SHORT: f32 = 0.05;
/// How fast the drift's size follows the body, a second.
const SWAY_RATE: f32 = 6.0;
/// How far along the ray the crosshair's point is taken, in metres. The ray's direction is what counts.
const CROSSHAIR_POINT: f32 = 10.0;
/// Lock-on, all my guesses: how far off the middle a creature may be, in radians, for the key and for the wheel.
const LOCK_CONE: f32 = 0.6;
const LOCK_CONE_WHEEL: f32 = 1.6;
/// How far it reaches, and how far before it lets go, in metres.
const LOCK_REACH: f32 = 25.0;
const LOCK_FAR: f32 = 30.0;
/// How long it may be out of sight, in seconds.
const LOCK_LOST_SECONDS: f32 = 2.0;
/// How fast the camera comes round to it, a second.
const LOCK_TURN_RATE: f32 = 5.0;
/// How far right of it the camera looks, in radians, or the player's own body stands in front of it.
const LOCK_ASIDE: f32 = 0.16;
/// Where on the body the mark goes, in metres over the feet.
const LOCK_MARK: f32 = 1.1;
/// What a power key says when it does nothing, over the player.
const NOT_KNOWN: &str = "not known";
const FORCE_NEEDED: &str = "Force points needed";
const NO_TARGET: &str = "no target";
const OUT_OF_REACH: &str = "out of reach";
/// After the stronger power's name: it is on, so this one would do nothing.
const STRONGER_ON: &str = "is on";

impl Stage {
    /// The aim button, down or up. It is the look button: holding it does nothing new until aiming may begin.
    pub fn set_aim_held(&mut self, held: bool) {
        self.aim_held = held;
    }

    /// Whether the player is aiming now. Clicks fire, and the window holds the cursor.
    pub fn aiming(&self) -> bool {
        self.aim_on
    }

    /// How far the camera has gone from plain to aiming, 0 to 1.
    pub fn aim_share(&self) -> f32 {
        self.aim_share
    }

    /// Whether the aim can begin or go on now.
    fn may_aim(&mut self) -> bool {
        // A blade blocks on this button and does not aim (`bladescreen`); a blaster, or bare hands, aim.
        self.aim_held
            && !(self.modern_mode() && self.game.world.blade_in_hand())
            && self.in_fight_now()
    }

    /// Whether Modern combat's fighting is up: in a fight, with an enemy in sight, locked on, or with Attack mode on.
    pub(super) fn in_fight_now(&self) -> bool {
        if !self.modern_combat() || self.menu_open() {
            return false;
        }
        let w = &self.game.world;
        let busy = w.in_dialogue()
            || w.opened.is_some()
            || w.game_over
            || self.turret_on()
            || w.in_race()
            || self.beat_camera().is_some()
            || w.object(w.pc).is_none_or(|o| o.down || o.dead);
        !busy
            && (self.party_blocked()
                || w.attack_mode()
                || self.lock_on.is_some()
                || self.seen_enemy().is_some())
    }

    /// Each frame of the game.
    pub(super) fn aim_tick(&mut self, dt: f32) {
        let on = self.may_aim();
        if on && !self.aim_on {
            // A click's walk ends: the keys steer from here.
            self.game.world.pc_mover.stop();
        }
        self.aim_on = on;
        self.aim_hot = false;
        let want = if on { 1.0 } else { 0.0 };
        self.aim_share += (want - self.aim_share) * (1.0 - (-AIM_RATE * dt).exp());
        if self.aim_share < 0.001 {
            self.aim_share = 0.0;
        }
        self.game.world.pc_aim = on.then_some(self.yaw);
        if on {
            let pc = self.game.world.pc;
            let w = &mut self.game.world;
            let since = w.time - (self.aim_next_shot - SHOT_SECONDS);
            let want = w.sway_size(since);
            self.aim_sway += (want - self.aim_sway) * (1.0 - (-SWAY_RATE * dt).exp());
            self.game.hold_stance(pc);
            // Rounds are not for the player while aiming: each shot is a click.
            self.game.world.stop_attacking(pc);
            let (eye, dir) = self.aim_ray();
            let reach = self.aim_reach(eye, dir);
            let who = self.game.world.aim_hit(eye, dir, reach).who;
            self.hover_target = who;
            self.aim_hot = who.is_some();
        } else {
            self.aim_sway = 0.0;
            if self.pitch < PLAIN_LOWEST {
                self.pitch += (PLAIN_LOWEST - self.pitch) * (1.0 - (-PITCH_BACK_RATE * dt).exp());
            }
        }
    }

    /// What the player is locked on to.
    pub fn locked_on(&self) -> Option<ObjId> {
        self.lock_on
    }

    /// Each creature the lock may go to, and how far round from the camera's middle it is, in radians.
    fn lock_round(&self, cone: f32) -> Vec<(ObjId, f32)> {
        let tau = std::f32::consts::TAU;
        let w = &self.game.world;
        let eye = self.camera().eye;
        let ids = w.objects.values().map(|o| (o.id, o.position));
        ids.filter(|(id, _)| w.may_lock(*id, LOCK_REACH))
            .map(|(id, at)| {
                let way = (at[1] - eye[1]).atan2(at[0] - eye[0]);
                let off = (way - self.yaw + tau / 2.0).rem_euclid(tau) - tau / 2.0;
                (id, off)
            })
            .filter(|(_, off)| off.abs() <= cone)
            .collect()
    }

    /// The lock-on key: the nearest to the middle of the view, or let go. Modern combat's only.
    pub fn lock_key(&mut self) {
        if self.lock_on.take().is_some() || !self.modern_combat() || self.menu_open() {
            return;
        }
        self.lock_lost = 0.0;
        let nearest = self
            .lock_round(LOCK_CONE)
            .into_iter()
            .min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()));
        self.lock_on = nearest.map(|n| n.0);
    }

    /// The wheel while locked: the next one round. Up goes left, as the view turns.
    pub fn lock_wheel(&mut self, lines: f32) {
        let Some(now) = self.lock_on else { return };
        let mut round = self.lock_round(LOCK_CONE_WHEEL);
        round.sort_by(|a, b| a.1.total_cmp(&b.1));
        let Some(place) = round.iter().position(|c| c.0 == now) else {
            return;
        };
        let next = if lines > 0.0 {
            round.get(place + 1)
        } else {
            place.checked_sub(1).and_then(|p| round.get(p))
        };
        if let Some(next) = next {
            self.lock_on = Some(next.0);
            self.lock_lost = 0.0;
        }
    }

    /// Each frame of the game: it lets go if it must, the camera comes round, and the body faces it. Aiming wins while it lasts.
    pub(super) fn lock_tick(&mut self, dt: f32) {
        let Some(id) = self.lock_on else { return };
        if self.menu_open() || self.paused() {
            return;
        }
        let w = &self.game.world;
        let me = w.pc_mover.pos;
        let it = w.object(id);
        let gone = !self.modern_combat()
            || w.in_dialogue()
            || w.game_over
            || w.object(w.pc).is_none_or(|o| o.down || o.dead)
            || it.is_none_or(|o| o.dead || o.down);
        let far =
            it.is_some_and(|o| (o.position[0] - me[0]).hypot(o.position[1] - me[1]) > LOCK_FAR);
        self.lock_lost = if w.may_lock(id, LOCK_FAR) {
            0.0
        } else {
            self.lock_lost + dt
        };
        if gone || far || self.lock_lost > LOCK_LOST_SECONDS {
            self.lock_on = None;
            return;
        }
        if self.aim_on {
            return;
        }
        let tau = std::f32::consts::TAU;
        let Some(at) = w.object(id).map(|o| o.position) else {
            return;
        };
        let eye = self.camera().eye;
        let way = (at[1] - eye[1]).atan2(at[0] - eye[0]);
        let off = (way - LOCK_ASIDE - self.yaw + tau / 2.0).rem_euclid(tau) - tau / 2.0;
        self.yaw += off * (1.0 - (-LOCK_TURN_RATE * dt).exp());
        self.game.world.pc_aim = Some((at[1] - me[1]).atan2(at[0] - me[0]));
        self.hover_target = Some(id);
    }

    /// The ray through the crosshair: where it starts, and its unit direction. It follows the drift the player sees.
    pub fn aim_ray(&self) -> ([f32; 3], [f32; 3]) {
        let c = self.camera();
        let unit = |v: [f32; 3]| {
            let long = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-4);
            [v[0] / long, v[1] / long, v[2] / long]
        };
        let f = unit([
            c.target[0] - c.eye[0],
            c.target[1] - c.eye[1],
            c.target[2] - c.eye[2],
        ]);
        let r = unit([f[1], -f[0], 0.0]);
        let u = [
            r[1] * f[2] - r[2] * f[1],
            r[2] * f[0] - r[0] * f[2],
            r[0] * f[1] - r[1] * f[0],
        ];
        let (a, b) = self.aim_drift();
        (c.eye, unit([0, 1, 2].map(|k| f[k] + r[k] * a + u[k] * b)))
    }

    /// How far the crosshair lies off the middle now, across and up, in radians.
    pub fn aim_drift(&self) -> (f32, f32) {
        let (a, b) = kotor_game::sway_path(self.game.world.time);
        (a * self.aim_sway, b * self.aim_sway)
    }

    /// How wide the crosshair drifts now, in radians.
    pub fn aim_sway(&self) -> f32 {
        self.aim_sway
    }

    /// Whether the crosshair lies on what a shot would hit.
    pub fn aim_hot(&self) -> bool {
        self.aim_hot
    }

    /// How far the ray goes before the room stops it.
    pub fn aim_reach(&self, eye: [f32; 3], dir: [f32; 3]) -> f32 {
        self.shell
            .first_hit(eye, dir, kotor_game::AIM_REACH)
            .unwrap_or(kotor_game::AIM_REACH)
    }

    /// A click while aiming fires, at the pace of the weapon. A blade or a bare hand does nothing yet. Not in a pause: there is
    /// no queue in Modern combat.
    pub(super) fn aim_click(&mut self) {
        let now = self.game.world.time;
        // A save from earlier in the game puts the clock back: a wait longer than one shot's is left from before it.
        let waiting = now < self.aim_next_shot && self.aim_next_shot - now <= SHOT_SECONDS;
        if self.paused() || waiting {
            return;
        }
        let (eye, dir) = self.aim_ray();
        let reach = self.aim_reach(eye, dir);
        let Some(hit) = self.game.world.fire_aimed(eye, dir, reach) else {
            return;
        };
        self.aim_next_shot = now + SHOT_SECONDS;
        let pc = self.game.world.pc;
        self.game.note_shot(pc);
        let w = &self.game.world;
        let (gun, pc) = (w.pc_gun(), w.pc);
        let to = [
            hit.point[0] - dir[0] * BOLT_SHORT,
            hit.point[1] - dir[1] * BOLT_SHORT,
            hit.point[2] - dir[2] * BOLT_SHORT,
        ];
        if let Some((bolt, color, shot)) = self.gun_look(pc) {
            self.effects.fire(gun, to, &bolt, color);
            if let Some(s) = shot {
                self.effect_from(&s, gun);
            }
        }
    }

    /// A key of the Force page: Modern combat's, at once, with no queue. The power goes to what the crosshair lies on, or to the
    /// player if it can. It gives back what it told the player, if it could not be cast.
    pub fn power_key(&mut self, action: &str) -> Option<String> {
        let power = super::menus::force_row(action)?;
        if !self.modern_combat() || self.menu_open() || self.paused() {
            return None;
        }
        let w = &mut self.game.world;
        if w.in_dialogue() || w.game_over {
            return None;
        }
        let pc = w.pc;
        let (_, range) = w.spell_cost_and_range(power)?;
        let (name, hostile) = w.power_row(power)?;
        let cost = w.power_cost(pc, power).unwrap_or(0);
        let o = w.object(pc)?;
        let (known, force) = (o.spells.contains(&power), o.force);
        if range == 0.0 && w.power_active(pc, power) {
            w.end_power(pc, power);
            return None;
        }
        let name = self.ui.strref(name);
        let stronger = self.game.world.stronger_on(pc, power);
        let stronger = stronger
            .and_then(|s| self.game.world.power_row(s))
            .map(|r| self.ui.strref(r.0));
        let on_crosshair = self.aim_on.then(|| {
            let (eye, dir) = self.aim_ray();
            let reach = self.aim_reach(eye, dir);
            self.game.world.aim_hit(eye, dir, reach).who
        });
        // Aiming wins over the lock while it lasts.
        let target = on_crosshair
            .flatten()
            .or(self.lock_on)
            .or((range == 0.0 || !hostile).then_some(pc));
        let refused = if !known {
            Some(format!("{name}: {NOT_KNOWN}"))
        } else if let Some(other) = stronger {
            Some(format!("{name}: {other} {STRONGER_ON}"))
        } else if force < cost {
            Some(format!("{name}: {cost} {FORCE_NEEDED}"))
        } else if target.is_none() {
            Some(format!("{name}: {NO_TARGET}"))
        } else {
            None
        };
        if refused.is_none() {
            if let Err(_why) = self.game.world.cast_spell(pc, power, target?, false) {
                let out = format!("{name}: {OUT_OF_REACH}");
                self.say_over_player(out.clone());
                return Some(out);
            }
        }
        if let Some(words) = &refused {
            self.say_over_player(words.clone());
        }
        refused
    }

    /// How high the camera looks, over the feet.
    pub(super) fn looks_at(&self) -> f32 {
        super::CAMERA_LOOKS_AT + (AIM_LOOKS_AT - super::CAMERA_LOOKS_AT) * self.aim_share
    }

    /// How much the aim takes off the view.
    pub(super) fn aim_narrows(&self) -> f32 {
        AIM_NARROWER * self.aim_share
    }

    /// How far the arm may reach as the aim comes in.
    pub(super) fn arm_reach(&self) -> f32 {
        self.distance + (AIM_ARM.min(self.distance) - self.distance) * self.aim_share
    }

    /// What the HUD draws for the aim: where the ray goes on the screen.
    pub fn crosshair_view(&self) -> Option<Crosshair> {
        if !self.aim_on {
            // Locked, it marks what it is locked on to.
            let at = self.game.world.object(self.lock_on?)?.position;
            let (x, y) = self.project(at, LOCK_MARK)?;
            return Some(Crosshair { x, y, hot: true });
        }
        let (eye, dir) = self.aim_ray();
        let (x, y) = self
            .project([0, 1, 2].map(|k| eye[k] + dir[k] * CROSSHAIR_POINT), 0.0)
            .unwrap_or((self.ui_width as i32 / 2, LAYOUT_HIGH / 2));
        Some(Crosshair {
            x,
            y,
            hot: self.aim_hot,
        })
    }
}
