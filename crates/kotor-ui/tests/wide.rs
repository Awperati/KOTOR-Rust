use kotor_archive::Install;
use kotor_ui::{DialogueView, Hit, HudMember, MenuView, Row, Ui};

const WIDTH: u32 = 853;

fn alpha(img: &kotor_image::Image, x: u32, y: u32) -> u8 {
    img.rgba[((y * img.width + x) * 4 + 3) as usize]
}

#[test]
fn a_picture_in_a_list_row_stays_square_on_a_wide_window() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mut ui = Ui::new(&mut inst);
    for (w, h) in [(1280, 720), (3840, 2160)] {
        ui.set_view((w, h));
        let draw = |ui: &mut Ui, icon: &str| {
            let mut v = MenuView::new("abilities");
            let row = Row {
                text: "A feat".into(),
                icon: Some(icon.into()),
                ..Row::default()
            };
            v.lists.insert("LB_ABILITY".into(), (vec![row], 0));
            ui.menu_overlay(&v).unwrap()
        };
        let with = draw(&mut ui, "po_pfha2");
        let without = draw(&mut ui, "no_such_picture");
        let (mut left, mut right, mut top, mut bottom) = (w, 0, h, 0);
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                if with.rgba[i..i + 4] != without.rgba[i..i + 4] {
                    left = left.min(x);
                    right = right.max(x);
                    top = top.min(y);
                    bottom = bottom.max(y);
                }
            }
        }
        let (wide, high) = (right + 1 - left, bottom + 1 - top);
        assert!(high > 20, "{high} high at {w}");
        assert!(wide.abs_diff(high) <= 3, "{wide} by {high} at {w}");
    }
}

#[test]
fn a_screen_is_the_same_after_going_to_a_size_with_upscaled_pictures_and_back() {
    let (Some(dir), Some(hd)) = (
        kotor_core::install_dir(),
        std::env::var_os("LOCALAPPDATA").map(|l| std::path::Path::new(&l).join("kotor-rs/hd")),
    ) else {
        return;
    };
    if !hd.is_dir() {
        return;
    }
    let mut inst = Install::open(dir).unwrap();
    let mut ui = Ui::new(&mut inst);
    ui.set_hd_dir(Some(hd));
    ui.set_view((1280, 720));
    let view = MenuView::new("abilities");
    let first = ui.menu_overlay(&view).unwrap();
    for _ in 0..3 {
        ui.set_view((3840, 2160));
        ui.menu_overlay(&view).unwrap();
        ui.set_view((1280, 720));
        let again = ui.menu_overlay(&view).unwrap();
        assert!(first.rgba == again.rgba, "the pictures changed places");
    }
}

#[test]
fn a_portrait_stays_square_on_a_wide_window() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mut ui = Ui::new(&mut inst);
    for (w, h) in [(1280, 720), (3840, 2160)] {
        ui.set_view((w, h));
        let mut with = MenuView::new("abilities");
        with.fill("LBL_PORTRAIT", "po_pfha2");
        let with = ui.menu_overlay(&with).unwrap();
        let without = ui.menu_overlay(&MenuView::new("abilities")).unwrap();
        let (mut left, mut right, mut top, mut bottom) = (w, 0, h, 0);
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                if with.rgba[i..i + 4] != without.rgba[i..i + 4] {
                    left = left.min(x);
                    right = right.max(x);
                    top = top.min(y);
                    bottom = bottom.max(y);
                }
            }
        }
        let (wide, high) = (right + 1 - left, bottom + 1 - top);
        assert!(high > 100, "{high} high at {w}");
        assert!(wide.abs_diff(high) <= 3, "{wide} by {high} at {w}");
    }
}

#[test]
fn the_interface_reaches_the_edges_of_a_wide_screen() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mut ui = Ui::new(&mut inst);
    ui.set_width(WIDTH);

    // The dialogue box runs from edge to edge, down to the bottom, over a clear scene.
    let view = DialogueView {
        speaker: "Trask".into(),
        subtitle: "A line.".into(),
        ..DialogueView::default()
    };
    let d = ui.dialogue_overlay(&view).unwrap();
    assert_eq!((d.width, d.height), (WIDTH, 480));
    for x in [0, WIDTH / 2, WIDTH - 1] {
        assert!(alpha(&d, x, 470) > 0, "bar at x {x}");
        assert_eq!(alpha(&d, x, 100), 0, "scene at x {x}");
    }

    // The party keeps its distance from the right edge: the portrait moved by all of the extra width.
    let party = [HudMember {
        portrait: "po_pfha2".into(),
        hp: 10,
        max_hp: 10,
        force: 5,
        max_force: 5,
        level_up: false,
    }];
    let h = ui.hud_overlay(&party, None, "", "").unwrap();
    assert_eq!((h.width, h.height), (WIDTH, 480));
    // The game's own layout: the portrait at the bottom left, the action slots at the bottom right.
    let (px, py) = ui.hud_point("LBL_CHAR1").unwrap();
    assert!(alpha(&h, px as u32, py as u32) > 0, "portrait");
    assert!(px < 80.0 && py > 400.0, "{px} {py}");
    let (ax, ay) = ui.hud_point("BTN_ACTION3").unwrap();
    assert!(ax > (WIDTH - 60) as f32 && ay > 400.0, "{ax} {ay}");
    assert!(alpha(&h, ax as u32, ay as u32) > 0, "action slot");
    assert_eq!(ui.hud_hit(ax, ay), None, "only the first slot is in use");
    let (bx, by) = ui.hud_point("BTN_OPT").unwrap();
    assert_eq!(ui.hud_hit(bx, by).as_deref(), Some("BTN_OPT"));
    assert_eq!(alpha(&h, WIDTH / 2, 240), 0, "the middle stays clear");
}

#[test]
fn a_stock_screen_draws_its_buttons_and_answers_what_is_under_the_cursor() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mut ui = Ui::new(&mut inst);
    // The strings on buttons come from `dialog.tlk`, which is a file beside the game's data.
    assert_eq!(ui.strref(1586), "New Game");
    ui.set_view((1280, 720));
    let mut view = MenuView::new("mainmenu");
    let (x, y) = ui.menu_point(&view, "BTN_NEWGAME", None).unwrap();
    assert_eq!(
        ui.menu_hit(&view, x, y),
        Some(Hit::Button("BTN_NEWGAME".into()))
    );
    assert_eq!(ui.menu_hit(&view, 1.0, 1.0), None);
    let plain = ui.menu_overlay(&view).unwrap();
    assert_eq!((plain.width, plain.height), (1280, 720));
    view.hover = Some(Hit::Button("BTN_NEWGAME".into()));
    assert_ne!(ui.menu_overlay(&view).unwrap().rgba, plain.rgba);
    view.hover = None;
    view.hide(&["BTN_NEWGAME"]);
    assert_eq!(ui.menu_hit(&view, x, y), None, "hidden: no button there");
    assert_ne!(ui.menu_overlay(&view).unwrap().rgba, plain.rgba);
}
