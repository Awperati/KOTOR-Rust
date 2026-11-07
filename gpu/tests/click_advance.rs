//! A conversation goes on with a click, through the path a player's mouse takes. The owner's note: "the playtester lacks the
//! ability to click on the screen to progress cutscenes".
use kotor_game::Autopilot;
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;
const SIZE: (u32, u32) = (1280, 720);

fn at_start(name: &str) -> Option<Stage> {
    kotor_core::install_dir()?;
    let mut s = Stage::open("end_m01aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let dir = std::env::temp_dir().join(format!("kotor_click_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.txt");
    std::fs::write(&file, "auto_advance=false\n").unwrap();
    s.use_settings_file(file);
    s.set_saves_dir(dir);
    Some(s)
}

/// Until a line is held whose replies are or are not all blank, taking the first reply of any other.
fn until_line(s: &mut Stage, blank: bool) -> bool {
    for _ in 0..30000 {
        s.update(DT);
        let w = &s.game.world;
        if let Some((_, _, _, replies)) = w.dialogue_view() {
            if w.conversation_paused() {
                continue;
            }
            if replies.iter().all(|r| r.trim().is_empty()) == blank {
                return true;
            }
            s.number_key(1);
            s.continue_line();
        }
    }
    false
}

fn pixel(s: &mut Stage, reply: usize) -> (f32, f32) {
    let (x, y) = s.line_point(reply).expect("a line is up");
    (
        x * SIZE.0 as f32 / s.ui_layout_width() as f32,
        y * SIZE.1 as f32 / 480.0,
    )
}

#[test]
fn a_cutscene_line_waits_for_the_click_and_goes_on_with_it() {
    let Some(mut s) = at_start("line") else {
        return;
    };
    assert!(until_line(&mut s, true), "no line with nothing to answer");
    let line = s.game.world.dialogue_serial();
    for _ in 0..600 {
        s.update(DT);
    }
    assert_eq!(
        s.game.world.dialogue_serial(),
        line,
        "it waits, with auto-advance off"
    );
    let at = pixel(&mut s, 0);
    s.click(at, SIZE);
    assert_ne!(
        s.game.world.dialogue_serial(),
        line,
        "the click moves it on"
    );
}

#[test]
fn a_line_with_answers_goes_on_only_with_a_click_on_one() {
    let Some(mut s) = at_start("reply") else {
        return;
    };
    assert!(until_line(&mut s, false), "no line with answers");
    let line = s.game.world.dialogue_serial();
    for _ in 0..300 {
        s.update(DT);
    }
    // The top of the window has no answer on it.
    s.click((640.0, 4.0), SIZE);
    assert_eq!(
        s.game.world.dialogue_serial(),
        line,
        "a click off the answers"
    );
    let at = pixel(&mut s, 0);
    s.hover(at, SIZE);
    s.click(at, SIZE);
    assert_ne!(s.game.world.dialogue_serial(), line, "a click on one");
}

/// The Endar Spire's opening, by the pilot, every line by the mouse.
#[test]
fn the_pilot_clicks_its_way_through_the_opening() {
    let Some(mut s) = at_start("pilot") else {
        return;
    };
    s.speed = 16.0;
    s.pilot = Some(Autopilot::new());
    let began = std::time::Instant::now();
    while s.game.world.module == "end_m01aa" && began.elapsed().as_secs() < 120 {
        s.update(DT);
    }
    let p = s.pilot.take().unwrap();
    assert_ne!(s.game.world.module, "end_m01aa", "it left the first module");
    assert!(p.lines_clicked > 5, "{} lines", p.lines_clicked);
    assert!(p.lines_not_clicked.is_empty(), "{:?}", p.lines_not_clicked);
}
