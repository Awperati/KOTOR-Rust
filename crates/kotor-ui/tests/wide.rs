use kotor_archive::Install;
use kotor_ui::{DialogueView, Hit, HudMember, MenuView, Ui};

const WIDTH: u32 = 853;

fn alpha(img: &kotor_image::Image, x: u32, y: u32) -> u8 {
    img.rgba[((y * img.width + x) * 4 + 3) as usize]
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
    }];
    let h = ui.hud_overlay(&party, None, "", "").unwrap();
    assert_eq!((h.width, h.height), (WIDTH, 480));
    let moved = WIDTH - 640;
    assert!(alpha(&h, 510 + moved, 405) > 0, "portrait at its new place");
    assert_eq!(alpha(&h, 510, 405), 0, "nothing at the old place");
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
