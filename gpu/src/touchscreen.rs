//! Fingers and the mouse on the on-screen controls. The window hands over what each finger does; this says what it has hold
//! of. Two sticks and buttons at once, each finger by its own id. A finger on nothing taps, or on the right half drags the camera.
//! On a screen, in a conversation, at the turret and in a race a finger does what the mouse or the keys do there.
//!
//! **Assumed:** a light push on the stick walks, under `WALK_BELOW`. A list scrolls one row for `SCROLL_SHARE` of the window.
//! The Attack button goes for the target if it is an enemy, else the nearest enemy seen. Attack mode has its own button.
//! The look stick at its middle speed and pushed all the way turns the camera `LOOK_TURN`, and tilts it `LOOK_TILT` of that.
//!
//! Modern combat adds Aim, held, Lock on, and Force, which opens a wheel of the first four powers the player knows, in the
//! Force page's order. Attack fires while aiming and strikes with a blade in a fight, again and again while held. **Assumed:**
//! which four powers, and that the wheel takes the four big buttons' places. See `docs/MODERN_COMBAT.md`.

use super::{slot_is_pickup, Stage};
use kotor_game::Kind;
use kotor_ui::{TouchBoxes, TouchKey, TouchPart, TouchView, MODERN_KEYS, TOUCH_KEYS};

/// The mouse has no finger's id: this is its own.
const MOUSE_FINGER: u64 = u64::MAX;
/// A finger that has come this far, as a share of the window's shorter side, has not tapped.
const TAP_SHARE: f32 = 0.02;
const SCROLL_SHARE: f32 = 0.06;
/// How far the camera turns for a drag is the mouse's at this window height, so that it does not fall off at bigger sizes.
const LOOK_HEIGHT: f32 = 720.0;
const STICK_DEAD: f32 = 0.15;
/// Radians a second, with the speed's slider at its middle, about what the keys that turn the camera do.
const LOOK_TURN: f32 = 2.0;
const LOOK_TILT: f32 = 0.5;
const LOOK_SLIDER_MIDDLE: f32 = 0.5;
const WALK_BELOW: f32 = 0.55;
/// The left and right parts of the window steer in a race. Between them is the gear.
const RACE_SIDE_SHARE: f32 = 1.0 / 3.0;

#[derive(Clone, Copy, PartialEq)]
enum Holds {
    Stick,
    Look,
    Key(TouchKey),
    /// A tap if it does not move. `looks`: the camera follows it when it does.
    Free {
        looks: bool,
        moved: bool,
    },
    /// On a screen: the drag not yet given to the list.
    Scroll(f32),
    Turret,
    /// Which way it steers.
    Race(f32),
    Nothing,
}

#[derive(Clone, Copy)]
struct Finger {
    id: u64,
    holds: Holds,
    start: (f32, f32),
    last: (f32, f32),
}

#[derive(Default)]
pub(super) struct Fingers {
    down: Vec<Finger>,
    stick: [f32; 2],
    look: [f32; 2],
    /// Modern combat's wheel of powers is open.
    wheel: bool,
    /// The wheel's powers: the Force page's action, the name, and whether the Force points are short for it.
    powers: Vec<(&'static str, String, bool)>,
    /// What the Attack and Aim buttons say in Modern combat, if not that.
    attack_word: &'static str,
    aim_word: &'static str,
}

/// Modern combat's words for the Attack and Aim buttons, as they do other things there.
const FIRE: &str = "Fire";
const STRIKE: &str = "Strike";
const GUARD: &str = "Guard";
/// The wheel has the first powers of the Force page the player knows, as many as this.
const WHEEL_POWERS: usize = 4;

impl Stage {
    /// The on-screen controls are on.
    pub fn touch_on(&self) -> bool {
        self.menus.settings.touch_controls
    }

    /// The blue button.
    pub fn toggle_touch(&mut self) {
        self.menus.settings.touch_controls = !self.menus.settings.touch_controls;
        if self
            .menus
            .touch
            .down
            .iter()
            .any(|f| f.holds == Holds::Key(TouchKey::Aim))
        {
            self.set_aim_held(false);
        }
        self.menus
            .touch
            .down
            .retain(|f| !matches!(f.holds, Holds::Stick | Holds::Look | Holds::Key(_)));
        self.menus.touch.stick = [0.0; 2];
        self.menus.touch.look = [0.0; 2];
        self.settings_changed();
        self.touch_set_view();
    }

    /// The HUD has the layer as it is now.
    fn touch_set_view(&mut self) {
        self.touch_refresh();
        let view = self.touch_view();
        self.ui.set_touch(view);
    }

    /// What Modern combat's buttons say and which powers the wheel has, kept for the view.
    fn touch_refresh(&mut self) {
        let modern = self.modern_combat();
        let fingers = &mut self.menus.touch;
        (fingers.attack_word, fingers.aim_word) = ("", "");
        if !modern {
            fingers.wheel = false;
            fingers.powers.clear();
            return;
        }
        let pc = self.game.world.pc;
        let blade = self.game.blade_stance(pc).is_some();
        let fight = self.in_fight_now();
        self.menus.touch.attack_word = if blade && fight {
            STRIKE
        } else if self.aiming() {
            FIRE
        } else {
            ""
        };
        self.menus.touch.aim_word = if blade { GUARD } else { "" };
        let w = &self.game.world;
        let me = w.object(pc);
        let known: Vec<(&'static str, i32)> = super::keyscreen::force_rows()
            .iter()
            .filter(|r| me.is_some_and(|o| o.spells.contains(&r.1)))
            .take(WHEEL_POWERS)
            .copied()
            .collect();
        let mut powers = Vec::new();
        for (action, power) in known {
            let w = &mut self.game.world;
            let name = w.power_row(power).map(|r| r.0);
            let cost = w.power_cost(pc, power).unwrap_or(0);
            let short = w.object(pc).is_some_and(|o| o.force < cost);
            let name = name.map(|n| self.ui.strref(n)).unwrap_or_default();
            powers.push((action, name, short));
        }
        self.menus.touch.powers = powers;
    }

    /// What the HUD draws of the layer.
    pub(in crate::stage) fn touch_view(&self) -> TouchView {
        let w = &self.game.world;
        let fingers = &self.menus.touch;
        let mut lit = Vec::new();
        if self.menus.paused {
            lit.push(TouchKey::Pause);
        }
        if w.attack_mode() {
            lit.push(TouchKey::AttackMode);
        }
        let mut dim = if self.menus.settings.movement_modern {
            Vec::new()
        } else {
            vec![TouchKey::Jump, TouchKey::Roll]
        };
        // A light push walks, and there is no jump at a walk.
        if matches!(self.touch_steer(), Some((_, _, true))) && !dim.contains(&TouchKey::Jump) {
            dim.push(TouchKey::Jump);
        }
        let modern = self.modern_combat();
        if modern {
            if self.lock_on.is_some() {
                lit.push(TouchKey::Lock);
            }
            if fingers.wheel {
                lit.push(TouchKey::Force);
            }
            if !self.in_fight_now() {
                dim.push(TouchKey::Aim);
            }
            for (n, p) in fingers.powers.iter().enumerate() {
                if p.2 {
                    dim.push(TouchKey::Power(n as u8));
                }
            }
        }
        TouchView {
            modern,
            wheel: modern && fingers.wheel,
            powers: fingers.powers.iter().map(|p| p.1.clone()).collect(),
            attack: fingers.attack_word.to_string(),
            aim: fingers.aim_word.to_string(),
            on: self.menus.settings.touch_controls,
            stick: fingers.stick,
            look: fingers.look,
            held: fingers
                .down
                .iter()
                .filter_map(|f| match f.holds {
                    Holds::Key(k) => Some(k),
                    _ => None,
                })
                .collect(),
            lit,
            dim,
            action: self.target_view_action().unwrap_or("Action").to_string(),
        }
    }

    fn layout_of(&self, at: (f32, f32), size: (u32, u32)) -> (f32, f32) {
        (
            at.0 * self.ui_width as f32 / size.0.max(1) as f32,
            at.1 * 480.0 / size.1.max(1) as f32,
        )
    }

    fn set_stick(&mut self, at: (f32, f32), size: (u32, u32)) {
        let (x, y) = self.layout_of(at, size);
        self.menus.touch.stick = self.ui.touch_stick_value(TouchPart::Stick, x, y);
    }

    fn set_look(&mut self, at: (f32, f32), size: (u32, u32)) {
        let (x, y) = self.layout_of(at, size);
        self.menus.touch.look = self.ui.touch_stick_value(TouchPart::Look, x, y);
    }

    /// Once a frame: the look stick turns the camera for as long as it is pushed, the faster the further. The Invert Camera box
    /// goes for it as for the mouse. A pause does not stop it, as it does not stop the mouse.
    pub(in crate::stage) fn touch_look_tick(&mut self, dt: f32) {
        let fingers = &self.menus.touch;
        if !fingers.down.iter().any(|f| f.holds == Holds::Look) {
            return;
        }
        let [right, down] = fingers.look;
        let push = right.hypot(down);
        if push < STICK_DEAD || self.menu_open() || self.choice_view().is_some() {
            return;
        }
        let s = self.settings();
        let strength = (push - STICK_DEAD) / (1.0 - STICK_DEAD) / push;
        let turn = LOOK_TURN * s.look_stick_speed / LOOK_SLIDER_MIDDLE * strength * dt;
        let flip = if s.invert_y { -1.0 } else { 1.0 };
        self.yaw -= right * turn;
        self.pitch = (self.pitch + down * flip * turn * LOOK_TILT).clamp(-0.2, 1.2);
    }

    /// What is under a finger of the layer, and what its going down does.
    fn take_part(&mut self, part: TouchPart, at: (f32, f32), size: (u32, u32)) -> Holds {
        match part {
            TouchPart::Stick => {
                self.set_stick(at, size);
                Holds::Stick
            }
            TouchPart::Look => {
                self.set_look(at, size);
                Holds::Look
            }
            TouchPart::Key(key) => {
                self.touch_key(key);
                Holds::Key(key)
            }
        }
    }

    fn touch_key(&mut self, key: TouchKey) {
        match key {
            TouchKey::Action => {
                let action = self.target_view_action();
                match self.target().filter(|_| !slot_is_pickup(action)) {
                    Some(id) => self.use_thing(id),
                    None => self.interact(),
                }
            }
            TouchKey::Attack if self.modern_fire() => {}
            TouchKey::Attack => {
                let w = &self.game.world;
                let is_foe = |id: &u32| {
                    w.object(*id)
                        .is_some_and(|o| o.kind == Kind::Creature && !o.dead)
                        && w.reputation(w.pc, *id) <= 10
                };
                let foe = self.target().filter(is_foe).or_else(|| self.seen_enemy());
                if let Some(foe) = foe {
                    self.hover_target = Some(foe);
                    self.use_thing(foe);
                }
            }
            TouchKey::Jump => self.jump_key(),
            TouchKey::Roll => self.roll_key(),
            TouchKey::Pickup => {
                self.hud_click_sound("");
                self.pickup();
            }
            TouchKey::Pause => {
                self.hud_click_sound("");
                self.pause_key();
            }
            TouchKey::AttackMode => {
                self.hud_click_sound("");
                self.attack_mode_key();
            }
            TouchKey::Aim => self.set_aim_held(true),
            TouchKey::Lock => {
                self.hud_click_sound("");
                self.lock_key();
            }
            TouchKey::Force => {
                self.hud_click_sound("");
                self.menus.touch.wheel = !self.menus.touch.wheel;
            }
            TouchKey::Power(n) => {
                self.menus.touch.wheel = false;
                let action = self.menus.touch.powers.get(n as usize).map(|p| p.0);
                if let Some(action) = action {
                    self.power_key(action);
                }
            }
        }
    }

    /// Modern combat's Attack button: a shot while aiming, a blow with a blade in a fight. Anything else is the button's as
    /// ever, and this says false.
    fn modern_fire(&mut self) -> bool {
        if !self.modern_combat() {
            return false;
        }
        if self.aiming() {
            if !self.blow_click() {
                self.aim_click();
            }
            return true;
        }
        self.blow_click()
    }

    /// Once a frame: Modern combat's buttons are kept up, and the Attack button held fires again, as fast as the weapon goes.
    pub(in crate::stage) fn touch_fire_tick(&mut self) {
        if !self.touch_on() {
            return;
        }
        self.touch_refresh();
        let held = |f: &Finger| f.holds == Holds::Key(TouchKey::Attack);
        if self.menus.touch.down.iter().any(held) {
            self.modern_fire();
        }
    }

    /// What a finger coming down has hold of.
    fn what_a_finger_takes(&mut self, at: (f32, f32), size: (u32, u32)) -> Holds {
        if self.turret_on() {
            self.turret_fire(true);
            return Holds::Turret;
        }
        if self.menu_open() || self.choice_view().is_some() {
            self.hover(at, size);
            self.click(at, size);
            return Holds::Scroll(0.0);
        }
        if self.game.world.in_race() {
            let side = size.0 as f32 * RACE_SIDE_SHARE;
            if at.0 >= side && at.0 < size.0 as f32 - side {
                self.jump_key();
                return Holds::Race(0.0);
            }
            return Holds::Race(if at.0 < side { -1.0 } else { 1.0 });
        }
        let (x, y) = self.layout_of(at, size);
        if let Some(part) = self.ui.touch_at(x, y) {
            return self.take_part(part, at, size);
        }
        if self.ui.hud_hit(x, y).is_some() {
            self.hover(at, size);
            self.click(at, size);
            return Holds::Nothing;
        }
        if let Some(stick) = self.ui.touch_stick_takes(x, y) {
            return self.take_part(stick, at, size);
        }
        let right_half = x * 2.0 >= self.ui_width as f32;
        Holds::Free {
            looks: right_half || !self.touch_on(),
            moved: false,
        }
    }

    /// A finger came down, at a pixel of a window of `size`.
    pub fn finger_down(&mut self, id: u64, at: (f32, f32), size: (u32, u32)) {
        self.finger_lost(id);
        self.touch_set_view();
        let holds = self.what_a_finger_takes(at, size);
        self.menus.touch.down.push(Finger {
            id,
            holds,
            start: at,
            last: at,
        });
    }

    /// A finger moved. Its id says which, and all the others stay as they are.
    pub fn finger_moved(&mut self, id: u64, at: (f32, f32), size: (u32, u32)) {
        let Some(n) = self.menus.touch.down.iter().position(|f| f.id == id) else {
            return;
        };
        let finger = self.menus.touch.down[n];
        let (dx, dy) = (at.0 - finger.last.0, at.1 - finger.last.1);
        self.menus.touch.down[n].last = at;
        let short = size.0.min(size.1).max(1) as f32;
        let look = LOOK_HEIGHT / short;
        match finger.holds {
            Holds::Stick => self.set_stick(at, size),
            Holds::Look => self.set_look(at, size),
            Holds::Free { looks, moved } => {
                let gone = (at.0 - finger.start.0).hypot(at.1 - finger.start.1);
                if moved || gone > short * TAP_SHARE {
                    self.menus.touch.down[n].holds = Holds::Free { looks, moved: true };
                    if looks {
                        self.look(dx * look, dy * look);
                    }
                }
            }
            Holds::Scroll(left) => {
                let step = size.1 as f32 * SCROLL_SHARE;
                let all = left + dy;
                let rows = (all / step).trunc();
                if rows != 0.0 && self.menu_open() {
                    self.menu_wheel(rows);
                }
                self.menus.touch.down[n].holds = Holds::Scroll(all - rows * step);
            }
            Holds::Turret => self.turret_look(dx * look, dy * look),
            Holds::Race(right) if right != 0.0 => {
                let side = size.0 as f32 * RACE_SIDE_SHARE;
                self.menus.touch.down[n].holds = Holds::Race(if at.0 < side {
                    -1.0
                } else if at.0 >= size.0 as f32 - side {
                    1.0
                } else {
                    right
                });
            }
            Holds::Key(_) | Holds::Race(_) | Holds::Nothing => {}
        }
    }

    /// A finger came up: one that did not move has tapped.
    pub fn finger_up(&mut self, id: u64, at: (f32, f32), size: (u32, u32)) {
        let held = self.menus.touch.down.iter().find(|f| f.id == id).copied();
        self.finger_lost(id);
        let Some(finger) = held else { return };
        if let Holds::Free { moved: false, .. } = finger.holds {
            if !self.menu_open() {
                self.hover(at, size);
                self.click(at, size);
            }
        }
        // A finger does not stay where it was, as a cursor does.
        self.set_hud_hover(None);
    }

    /// A finger is gone, with no tap: it came up off the window, or the window lost it.
    pub fn finger_lost(&mut self, id: u64) {
        let Some(n) = self.menus.touch.down.iter().position(|f| f.id == id) else {
            return;
        };
        match self.menus.touch.down.remove(n).holds {
            Holds::Stick => self.menus.touch.stick = [0.0; 2],
            Holds::Look => self.menus.touch.look = [0.0; 2],
            Holds::Turret => self.turret_fire(false),
            Holds::Key(TouchKey::Aim) => self.set_aim_held(false),
            _ => {}
        }
    }

    /// The window lost the input: no finger is down any more.
    pub fn fingers_gone(&mut self) {
        let ids: Vec<u64> = self.menus.touch.down.iter().map(|f| f.id).collect();
        for id in ids {
            self.finger_lost(id);
        }
    }

    /// A mouse button came down: if on the stick or a round button, that is what it has hold of. A click anywhere else is
    /// the mouse's as ever.
    pub fn mouse_down_on_layer(&mut self, at: (f32, f32), size: (u32, u32)) -> bool {
        if !self.touch_on()
            || self.menu_open()
            || self.choice_view().is_some()
            || self.turret_on()
            || self.game.world.in_race()
        {
            return false;
        }
        self.touch_set_view();
        let (x, y) = self.layout_of(at, size);
        let Some(part) = self.ui.touch_at(x, y) else {
            return false;
        };
        let holds = self.take_part(part, at, size);
        self.finger_lost(MOUSE_FINGER);
        self.menus.touch.down.push(Finger {
            id: MOUSE_FINGER,
            holds,
            start: at,
            last: at,
        });
        true
    }

    /// The cursor moved. Only what the mouse has hold of of the layer cares.
    pub fn mouse_moved_on_layer(&mut self, at: (f32, f32), size: (u32, u32)) {
        self.finger_moved(MOUSE_FINGER, at, size);
    }

    /// The mouse button came up.
    pub fn mouse_up_on_layer(&mut self) {
        self.finger_lost(MOUSE_FINGER);
    }

    /// What the stick, or a finger in a race, asks of the movement: ahead, right, and whether it walks. Nothing if neither is
    /// held, and the keys have it.
    pub fn touch_steer(&self) -> Option<(f32, f32, bool)> {
        let fingers = &self.menus.touch;
        let mut asked = None;
        for f in &fingers.down {
            match f.holds {
                Holds::Race(right) => asked = Some((0.0, right, false)),
                Holds::Stick => {
                    let [right, down] = fingers.stick;
                    let push = right.hypot(down);
                    asked = Some(if push < STICK_DEAD {
                        (0.0, 0.0, false)
                    } else {
                        (-down, right, push < WALK_BELOW)
                    });
                }
                _ => {}
            }
        }
        asked
    }

    /// A finger is turning the camera.
    pub fn touch_looking(&self) -> bool {
        let [right, down] = self.menus.touch.look;
        let pushed = right.hypot(down) >= STICK_DEAD;
        let looking = |f: &Finger| match f.holds {
            Holds::Look => pushed,
            Holds::Free {
                looks: true,
                moved: true,
            } => true,
            _ => false,
        };
        self.menus.touch.down.iter().any(looking)
    }

    /// Whether a round button is greyed now, for a test.
    pub fn touch_key_greyed(&self, key: TouchKey) -> bool {
        self.touch_view().dim.contains(&key)
    }

    /// What a round button says, and whether it is lit, for a test.
    pub fn touch_key_word(&mut self, key: TouchKey) -> (String, bool) {
        self.touch_refresh();
        let view = self.touch_view();
        let word = match key {
            TouchKey::Attack if !view.attack.is_empty() => view.attack.clone(),
            TouchKey::Aim if !view.aim.is_empty() => view.aim.clone(),
            TouchKey::Power(n) => view.powers.get(n as usize).cloned().unwrap_or_default(),
            other => format!("{other:?}"),
        };
        (word, view.lit.contains(&key) || view.held.contains(&key))
    }

    /// How many fingers are down, for a test.
    pub fn fingers_down(&self) -> usize {
        self.menus.touch.down.len()
    }

    /// The boxes of the layer and of what it must stay off, for a test.
    pub fn touch_boxes(&mut self) -> Option<TouchBoxes> {
        self.touch_set_view();
        self.ui.touch_boxes()
    }

    /// The pixel of a window of `size` in the middle of a part of the layer, for a test or a driver.
    pub fn touch_pixel(&mut self, part: TouchPart, size: (u32, u32)) -> Option<(f32, f32)> {
        self.touch_set_view();
        let (x, y) = self.ui.touch_point(part)?;
        Some((
            x * size.0 as f32 / self.ui_width as f32,
            y * size.1 as f32 / 480.0,
        ))
    }

    /// For a driver's state line: on or off, the stick, how many fingers, and where each part is in layout units.
    pub(in crate::stage) fn touch_state(&mut self) -> String {
        let layout = (self.ui_width, 480);
        let mut places = Vec::new();
        let sticks = [("stick", TouchPart::Stick), ("look", TouchPart::Look)];
        let keys = [
            "action", "attack", "jump", "roll", "pickup", "pause", "mode",
        ]
        .into_iter()
        .zip(TOUCH_KEYS.map(TouchPart::Key));
        let modern = [
            "aim", "lock", "force", "power0", "power1", "power2", "power3",
        ]
        .into_iter()
        .zip(MODERN_KEYS.map(TouchPart::Key));
        for (name, part) in sticks.into_iter().chain(keys).chain(modern) {
            if let Some(p) = self.touch_pixel(part, layout).filter(|_| self.touch_on()) {
                places.push(format!("{name}:{:.0}:{:.0}", p.0, p.1));
            }
        }
        let [right, down] = self.menus.touch.stick;
        let [look_right, look_down] = self.menus.touch.look;
        format!(
            "touch={} stick={right:.2},{down:.2} lookstick={look_right:.2},{look_down:.2} fingers={} touchat={}",
            self.touch_on(),
            self.fingers_down(),
            if places.is_empty() {
                "-".to_string()
            } else {
                places.join(";")
            }
        )
    }
}
