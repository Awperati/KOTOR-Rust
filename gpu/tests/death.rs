//! The player's death as the stage shows it: the fall with the camera held, the fade, the game-over screen and its two
//! buttons.
use kotor_game::{EffectRec, DURATION_INSTANT};
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 30.0;
const SIZE: (u32, u32) = (1280, 720);

fn press(s: &mut Stage, tag: &str) {
    let p = s
        .menu_pixel(tag, None, SIZE)
        .unwrap_or_else(|| panic!("no {tag} on {}", s.menu_name()));
    s.hover(p, SIZE);
    s.click(p, SIZE);
}

/// A new game past its opening, with a folder of its own for saves and one save in it.
fn playing(name: &str) -> Option<(Stage, std::path::PathBuf)> {
    kotor_core::install_dir()?;
    let mut s = Stage::menu("end_m01aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let dir = std::env::temp_dir().join(format!("kotor_rs_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    s.set_saves_dir(dir.clone());
    press(&mut s, "BTN_NEWGAME");
    for _ in 0..12000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        } else if s.game.world.transcript.len() > 3 && s.game.world.delayed.is_empty() {
            break;
        }
    }
    s.game
        .world
        .save_to(&dir.join("autosave.sav"), "Autosave")
        .unwrap();
    Some((s, dir))
}

fn fell(s: &mut Stage) {
    let w = &mut s.game.world;
    let party = w.standing_party();
    for id in party {
        w.object_mut(id).unwrap().min_one_hp = false;
        let mut e = EffectRec::new("EffectDamage");
        e.nums = vec![1000.0, 0.0, 0.0];
        w.effects.push(e);
        let h = w.effects.len() as u32;
        w.apply_effect(id, id, h, DURATION_INSTANT, 0.0);
    }
}

#[test]
fn the_party_falls_the_screen_fades_and_a_game_over_screen_offers_load_and_quit() {
    let Some((mut s, dir)) = playing("death") else {
        return;
    };
    let at = s.game.world.object(s.game.world.pc).unwrap().position;
    let eye = s.camera().eye;
    fell(&mut s);
    s.steer = (1.0, 0.0, false);
    s.update(DT);
    s.escape();
    // Falling: no menu, no walking, no pause menu on Esc, the camera where it was, the clock running.
    for _ in 0..(2.0 / DT) as usize {
        s.update(DT);
    }
    assert_eq!(s.menu_name(), "-");
    assert_eq!(s.game.world.object(s.game.world.pc).unwrap().position, at);
    assert_eq!(s.camera().eye, eye, "the camera holds");
    // Fading: the overlay is black and part clear.
    for _ in 0..(2.0 / DT) as usize {
        s.update(DT);
    }
    let alpha = s.overlay().unwrap().rgba[3];
    assert!(alpha > 0, "the picture is going dark: {alpha}");
    for _ in 0..(2.5 / DT) as usize {
        s.update(DT);
    }
    assert_eq!(s.menu_name(), "gameover");
    s.escape();
    assert_eq!(s.menu_name(), "gameover", "Esc does not leave it");
    // Load Game lists the saves, newest chosen, and loading puts the party back on its feet.
    press(&mut s, "BTN_OK");
    assert_eq!(s.menu_name(), "load");
    assert_eq!(s.saves_listed(), 1);
    press(&mut s, "BTN_SAVELOAD");
    assert_eq!(s.menu_name(), "-", "{}", s.menu_message());
    assert!(!s.game.world.game_over);
    assert!(!s.game.world.standing_party().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn quit_on_the_game_over_screen_goes_to_the_main_menu() {
    let Some((mut s, dir)) = playing("death_quit") else {
        return;
    };
    fell(&mut s);
    for _ in 0..(6.0 / DT) as usize {
        s.update(DT);
    }
    assert_eq!(s.menu_name(), "gameover");
    press(&mut s, "BTN_CANCEL");
    assert_eq!(s.menu_name(), "main");
    let _ = std::fs::remove_dir_all(dir);
}
