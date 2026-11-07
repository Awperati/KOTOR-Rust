//! Fingers and the mouse on the on-screen controls. The window hands over what each finger does; this says what it has hold
//! of. A stick and buttons at once, each finger by its own id. A finger on nothing taps, or on the right half drags the camera.
//! On a screen, in a conversation, at the turret and in a race a finger does what the mouse or the keys do there.
//!
//! **Assumed:** a light push on the stick walks, under `WALK_BELOW`. A list scrolls one row for `SCROLL_SHARE` of the window.
//! The Attack button goes for the target if it is an enemy, else the nearest enemy seen. Attack mode has its own button.

use super::{slot_is_pickup, Stage};
use kotor_game::Kind;
use kotor_ui::{TouchBoxes, TouchKey, TouchPart, TouchView, TOUCH_KEYS};

/// The mouse has no finger's id: this is its own.
const MOUSE_FINGER: u64 = u64::MAX;
/// A finger that has come this far, as a share of the window's shorter side, has not tapped.
const TAP_SHARE: f32 = 0.02;
const SCROLL_SHARE: f32 = 0.06;
/// How far the camera turns for a drag is the mouse's at this window height, so that it does not fall off at bigger sizes.
const LOOK_HEIGHT: f32 = 720.0;
const STICK_DEAD: f32 = 0.15;
const WALK_BELOW: f32 = 0.55;
/// The left and right parts of the window steer in a race. Between them is the gear.
const RACE_SIDE_SHARE: f32 = 1.0 / 3.0;

#[derive(Clone, Copy, PartialEq)]
enum Holds {
    Stick,
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
}

impl Stage {
    /// The on-screen controls are on.
    pub fn touch_on(&self) -> bool {
        self.menus.settings.touch_controls
    }

    /// The blue button.
    pub fn toggle_touch(&mut self) {
        self.menus.settings.touch_controls = !self.menus.settings.touch_controls;
        self.menus
            .touch
            .down
            .retain(|f| !matches!(f.holds, Holds::Stick | Holds::Key(_)));
        self.menus.touch.stick = [0.0; 2];
        self.settings_changed();
        let view = self.touch_view();
        self.ui.set_touch(view);
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
        let dim = if self.menus.settings.movement_modern {
            Vec::new()
        } else {
            vec![TouchKey::Jump, TouchKey::Roll]
        };
        TouchView {
            on: self.menus.settings.touch_controls,
            stick: fingers.stick,
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
        self.menus.touch.stick = self.ui.touch_stick_value(x, y);
    }

    /// What is under a finger of the layer, and what its going down does.
    fn take_part(&mut self, part: TouchPart, at: (f32, f32), size: (u32, u32)) -> Holds {
        match part {
            TouchPart::Stick => {
                self.set_stick(at, size);
                Holds::Stick
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
        if self.ui.touch_stick_takes(x, y) {
            return self.take_part(TouchPart::Stick, at, size);
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
        let view = self.touch_view();
        self.ui.set_touch(view);
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
            Holds::Turret => self.turret_fire(false),
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
        let view = self.touch_view();
        self.ui.set_touch(view);
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
        let looking = |f: &Finger| {
            matches!(
                f.holds,
                Holds::Free {
                    looks: true,
                    moved: true
                }
            )
        };
        self.menus.touch.down.iter().any(looking)
    }

    /// How many fingers are down, for a test.
    pub fn fingers_down(&self) -> usize {
        self.menus.touch.down.len()
    }

    /// The boxes of the layer and of what it must stay off, for a test.
    pub fn touch_boxes(&mut self) -> Option<TouchBoxes> {
        let view = self.touch_view();
        self.ui.set_touch(view);
        self.ui.touch_boxes()
    }

    /// The pixel of a window of `size` in the middle of a part of the layer, for a test or a driver.
    pub fn touch_pixel(&mut self, part: TouchPart, size: (u32, u32)) -> Option<(f32, f32)> {
        let view = self.touch_view();
        self.ui.set_touch(view);
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
        for (name, part) in std::iter::once(("stick", TouchPart::Stick)).chain(
            [
                "action", "attack", "jump", "roll", "pickup", "pause", "mode",
            ]
            .into_iter()
            .zip(TOUCH_KEYS.map(TouchPart::Key)),
        ) {
            if let Some(p) = self.touch_pixel(part, layout).filter(|_| self.touch_on()) {
                places.push(format!("{name}:{:.0}:{:.0}", p.0, p.1));
            }
        }
        let [right, down] = self.menus.touch.stick;
        format!(
            "touch={} stick={right:.2},{down:.2} fingers={} touchat={}",
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
