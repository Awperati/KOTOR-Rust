use kotor_archive::Install;
use kotor_ui::{parse_font_txi, DialogueView, Font, HudMember, Ui};

fn install() -> Option<Install> {
    Some(Install::open(kotor_core::install_dir()?).unwrap())
}

#[test]
fn font_txi_cells_are_read_in_pairs() {
    let txi = "numchars 2\nupperleftcoords 2\n0.0 1.0 0\n0.5 1.0 0\nlowerrightcoords 2\n0.25 0.5 0\n0.75 0.5 0\n";
    let cells = parse_font_txi(txi);
    assert_eq!(
        cells,
        vec![([0.0, 1.0], [0.25, 0.5]), ([0.5, 1.0], [0.75, 0.5])]
    );
}

#[test]
fn text_wraps_at_spaces_and_measures_by_glyph_width() {
    // Every cell is 4 px wide in a 64 px picture, so a string is 4 px per character.
    let img = kotor_image::Image {
        width: 64,
        height: 64,
        rgba: vec![255; 64 * 64 * 4],
    };
    let mut txi = String::from("upperleftcoords 256\n");
    for _ in 0..256 {
        txi.push_str("0.0 1.0 0\n");
    }
    txi.push_str("lowerrightcoords 256\n");
    for _ in 0..256 {
        txi.push_str("0.0625 0.9375 0\n");
    }
    let f = Font::from_parts("t", img, &txi);
    assert_eq!(f.measure("abcd"), 16);
    assert_eq!(f.line_height, 4);
    assert_eq!(f.wrap("aa bb cc dd", 20), vec!["aa bb", "cc dd"]);
    assert_eq!(f.wrap("one\ntwo", 100), vec!["one", "two"]);
    // A word wider than the line still gets a line of its own.
    assert_eq!(f.wrap("abcdefghij k", 12), vec!["abcdefghij", "k"]);
}

#[test]
fn dialog_gui_parses_with_its_message_and_reply_list() {
    let Some(mut inst) = install() else { return };
    let mut ui = Ui::new(&mut inst);
    let g = ui.gui("dialog").expect("dialog.gui");
    let e = &g.panel.extent;
    assert_eq!((e.left, e.top, e.width, e.height), (48, 378, 544, 100));
    let msg = g.find("LBL_MESSAGE").unwrap();
    assert_eq!(msg.text.as_ref().unwrap().font, "fnt_d16x16");
    let list = g.find("LB_REPLIES").unwrap();
    assert_eq!(list.proto_item.as_ref().unwrap().extent.width, 528);
}

#[test]
fn fonts_load_and_a_mismatched_one_falls_back() {
    let Some(mut inst) = install() else { return };
    let mut ui = Ui::new(&mut inst);
    let console = ui.font("fnt_console").expect("fnt_console").clone();
    assert!(console.has_ink_for("Ae0"));
    // fnt_d16x16's TXI does not describe its picture; the loader hands back a font that does.
    let dialog = ui.font("fnt_d16x16").expect("fallback").clone();
    assert!(dialog.has_ink_for("Ae0"));
    assert!(dialog.measure("Hello") > 20);
}

fn bright(img: &kotor_image::Image, x0: u32, y0: u32, x1: u32, y1: u32) -> usize {
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * img.width + x) * 4) as usize;
            if img.rgba[i] as u32 + img.rgba[i + 1] as u32 + img.rgba[i + 2] as u32 > 300 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn the_dialogue_box_draws_text_only_inside_its_panel() {
    let Some(mut inst) = install() else { return };
    let mut ui = Ui::new(&mut inst);
    let view = DialogueView {
        speaker: "Trask".into(),
        subtitle: "We've been ambushed by a Sith battle fleet!".into(),
        ..DialogueView::default()
    };
    let img = ui.dialogue_box(&view, None).unwrap();
    assert_eq!((img.width, img.height), (640, 480));
    assert_eq!(bright(&img, 0, 0, 640, 340), 0, "nothing above the panel");
    assert!(
        bright(&img, 48, 378, 592, 478) > 100,
        "subtitle text should be in the panel"
    );
}

#[test]
fn the_selected_reply_is_drawn_yellow_and_the_rest_white() {
    let Some(mut inst) = install() else { return };
    let mut ui = Ui::new(&mut inst);
    let view = DialogueView {
        replies: vec!["First reply".into(), "Second reply".into()],
        selected: 1,
        ..DialogueView::default()
    };
    let img = ui.dialogue_box(&view, None).unwrap();
    let yellow = |y0: u32, y1: u32| {
        let mut n = 0;
        for y in y0..y1 {
            for x in 48..592 {
                let i = ((y * 640 + x) * 4) as usize;
                if img.rgba[i] > 200 && img.rgba[i + 1] > 200 && img.rgba[i + 2] < 120 {
                    n += 1;
                }
            }
        }
        n
    };
    // Rows are one line tall (the font's line height) starting at the panel top, 378.
    let line = ui.font("fnt_d16x16").unwrap().line_height as u32;
    assert_eq!(yellow(378, 378 + line), 0, "first reply is not selected");
    assert!(
        yellow(378 + line, 378 + 2 * line + 2) > 20,
        "second reply is highlighted"
    );
}

#[test]
fn the_hud_shows_portraits_and_bars_proportionally() {
    let Some(mut inst) = install() else { return };
    let mut ui = Ui::new(&mut inst);
    let member = |hp| HudMember {
        portrait: "po_pfha1".into(),
        hp,
        max_hp: 100,
        force: 0,
        max_force: 100,
        level_up: false,
    };
    let red = |img: &kotor_image::Image| {
        let mut n = 0;
        // The first member's vitality bar: x 466..482, y 375..439 in layout space.
        for y in 375..439 {
            for x in 466..482 {
                let i = ((y * 640 + x) * 4) as usize;
                if img.rgba[i] > 150 && img.rgba[i + 1] < 80 && img.rgba[i + 2] < 80 {
                    n += 1;
                }
            }
        }
        n
    };
    let full = ui.hud(&[member(100)], None).unwrap();
    let half = ui.hud(&[member(50)], None).unwrap();
    let (a, b) = (red(&full), red(&half));
    assert!(a > 100, "a full bar has red pixels ({a})");
    assert!(
        b * 10 < a * 7 && b * 10 > a * 3,
        "half health should show roughly half the red: {b} of {a}"
    );
    // The portrait is drawn: the slot is not flat colour.
    let mut seen = std::collections::HashSet::new();
    for y in 380..434 {
        for x in 485..539 {
            let i = ((y * 640 + x) * 4) as usize;
            seen.insert([full.rgba[i], full.rgba[i + 1], full.rgba[i + 2]]);
        }
    }
    assert!(seen.len() > 50, "portrait colours: {}", seen.len());
}
