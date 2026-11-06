//! The player's death: the party falls with the camera held and the world still, the picture fades to black, and a
//! game-over screen offers Load Game or Quit. The logic is `World::death_phase`; this is what is shown and the buttons.
//!
//! **Assumed, not measured:** the original's timings, its wording, and that Load Game lists the saves newest first
//! with the newest chosen. The screen is `confirm.gui`, which has two buttons.

use super::menus::{Mode, Screen};
use super::Stage;
use kotor_game::DeathPhase;
use kotor_ui::MenuView;

const STR_LOAD_GAME: u32 = 1585;
const MESSAGE: &str = "You have died.";
const QUIT: &str = "Quit";

impl Stage {
    /// While the party is falling or fading: the game's clock runs (the fall plays), the camera stays and the picture
    /// goes black. When it is black the game-over screen goes up. Returns whether it took the frame.
    pub(super) fn death_tick(&mut self, dt: f32) -> bool {
        let Some(phase) = self.game.world.death_phase() else {
            return false;
        };
        if !self.started {
            return false;
        }
        let speed = self.speed;
        match phase {
            DeathPhase::Over => {
                self.menus.stack.push(Screen::GameOver);
                self.menus.hover = None;
                self.refresh_overlay();
                return true;
            }
            DeathPhase::Falling | DeathPhase::Fading(_) => {
                self.game.advance_at(dt, speed);
            }
        }
        let alpha = match phase {
            DeathPhase::Fading(f) => (f.clamp(0.0, 1.0) * 255.0) as u8,
            _ => 0,
        };
        // Nothing of the HUD while the party is down: a black picture, as clear as the fade has got.
        let key = format!("death{alpha}{}", self.ui_width);
        if key != self.overlay_key {
            let black = MenuView {
                over_scene: false,
                ..MenuView::default()
            };
            self.overlay = self.ui.menu_overlay(&black).map(|mut o| {
                for p in o.rgba.chunks_mut(4) {
                    p[3] = alpha;
                }
                o
            });
            self.overlay_key = key;
            self.overlay_version += 1;
        }
        true
    }

    pub(super) fn game_over_view(&mut self) -> MenuView {
        let mut v = MenuView::new("confirm");
        v.text("LB_MESSAGE", MESSAGE.to_string());
        v.font("LB_MESSAGE", "fnt_d16x16");
        let load = self.ui.strref(STR_LOAD_GAME);
        v.text("BTN_OK", load);
        v.text("BTN_CANCEL", QUIT.to_string());
        v.over_scene = false;
        v
    }

    pub(super) fn game_over_press(&mut self, tag: &str) {
        match tag {
            "BTN_OK" => {
                self.open_saves(Mode::Load);
                if self.saves_listed() > 0 {
                    self.menus.selected = Some(0);
                }
            }
            "BTN_CANCEL" => self.quit_to_main(),
            _ => {}
        }
    }
}
