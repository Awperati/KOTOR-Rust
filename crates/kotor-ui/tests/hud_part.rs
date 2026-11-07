//! Drawing part of the HUD again gives the picture that drawing the whole would.
use kotor_archive::Install;
use kotor_image::Image;
use kotor_ui::{CombatHud, Float, HudMember, MiniMap, Ui};

fn party(hp: i32) -> Vec<HudMember> {
    vec![HudMember {
        portrait: "po_pcarth".to_string(),
        hp,
        max_hp: 20,
        force: 0,
        max_force: 0,
        level_up: false,
    }]
}

fn float(x: i32, y: i32, text: &str) -> Float {
    Float {
        x,
        y,
        text: text.to_string(),
        color: [1.0, 0.2, 0.2],
    }
}

fn map(x: f32, facing: f32) -> Option<MiniMap> {
    Some(MiniMap {
        picture: "lbl_mapm01aa".to_string(),
        at: [x, 120.0],
        facing,
    })
}

/// All that a HUD is drawn from.
#[derive(Clone)]
struct Hud {
    party: Vec<HudMember>,
    target: Option<(&'static str, i32, i32)>,
    at: Option<(i32, i32)>,
    map: Option<MiniMap>,
    floats: Vec<Float>,
    combat: CombatHud,
    notices: Vec<String>,
}

impl Hud {
    fn set(&self, ui: &mut Ui) {
        ui.set_target_at(self.at);
        ui.set_minimap(self.map.clone());
        ui.set_floats(self.floats.clone());
        ui.set_combat(self.combat.clone());
        ui.set_notices(self.notices.clone());
    }

    fn whole(&self, ui: &mut Ui) -> Image {
        self.set(ui);
        ui.hud_overlay(&self.party, self.target, "Attack", "i_attack")
            .unwrap()
    }
}

/// From a HUD to the next, part of it drawn again is what the whole would be, and nothing outside the box it gave changed.
fn same_as_whole(ui: &mut Ui, view: (u32, u32), from: &Hud, to: &Hud, what: &str) {
    let first = from.whole(ui);
    to.set(ui);
    let (part, changed) = ui
        .hud_part(first.clone(), &to.party, to.target, "Attack", "i_attack")
        .unwrap_or_else(|| panic!("{what}: part of it could not be drawn"));
    let fresh = to.whole(ui);
    assert_ne!(first.rgba, fresh.rgba, "{what}: something has to change");
    let row = view.0 as usize * 4;
    let differ = part
        .rgba
        .chunks(row)
        .zip(fresh.rgba.chunks(row))
        .enumerate()
        .find_map(|(y, (a, b))| {
            a.chunks(4)
                .zip(b.chunks(4))
                .position(|(p, q)| p != q)
                .map(|x| (x, y))
        });
    assert_eq!(differ, None, "{what} at {view:?}");
    let [left, top, wide, high] = changed.map(|v| v as usize);
    assert!(
        wide * high < view.0 as usize * view.1 as usize,
        "{what} at {view:?}: {changed:?} is all of it"
    );
    let inside = |x: usize, y: usize| x >= left && x < left + wide && y >= top && y < top + high;
    for (y, (a, b)) in first
        .rgba
        .chunks(row)
        .zip(part.rgba.chunks(row))
        .enumerate()
    {
        for (x, (p, q)) in a.chunks(4).zip(b.chunks(4)).enumerate() {
            assert!(
                p == q || inside(x, y),
                "{what}: ({x}, {y}) is outside {changed:?}"
            );
        }
    }
}

#[test]
fn a_part_of_the_hud_drawn_again_is_what_the_whole_would_be() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mut ui = Ui::new(&mut inst);
    let start = Hud {
        party: party(10),
        target: Some(("Sith Trooper", 5, 10)),
        at: Some((300, 150)),
        map: map(100.0, 0.0),
        floats: vec![
            float(300, 200, "-12"),
            float(500, 90, "a long line that has to wrap to fit"),
        ],
        combat: CombatHud::default(),
        notices: Vec::new(),
    };
    let moved = Hud {
        at: Some((420, 230)),
        map: map(130.0, 1.0),
        floats: vec![float(310, 180, "-12"), float(120, 300, "Miss")],
        ..start.clone()
    };
    let hurt = Hud {
        party: party(3),
        ..start.clone()
    };
    let target_hurt = Hud {
        target: Some(("Sith Trooper", 1, 10)),
        ..start.clone()
    };
    let another = Hud {
        target: Some(("A much longer name than anybody's bar has room for", 7, 10)),
        ..start.clone()
    };
    let no_target = Hud {
        target: None,
        at: None,
        ..start.clone()
    };
    let fighting = Hud {
        combat: CombatHud {
            engaged: true,
            message: "Round 3 of a fight".to_string(),
            attack: Some("Attack".to_string()),
            ..CombatHud::default()
        },
        ..start.clone()
    };
    let noticed = Hud {
        notices: vec!["LBL_JOURNAL".to_string()],
        ..start.clone()
    };
    for view in [(1280, 720), (3840, 2160)] {
        ui.set_view(view);
        for (next, what) in [
            (&moved, "the map, the words and the target moved"),
            (&hurt, "the leader hurt"),
            (&target_hurt, "the target hurt"),
            (&another, "another target"),
            (&no_target, "no target"),
            (&fighting, "a fight"),
            (&noticed, "a notice"),
        ] {
            same_as_whole(&mut ui, view, &start, next, what);
            same_as_whole(&mut ui, view, next, &start, what);
        }
    }
}

#[test]
fn a_cell_seen_on_the_map_changes_the_picture_as_making_it_again_would() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mut ui = Ui::new(&mut inst);
    let (cols, rows) = (40, 20);
    let mut seen = vec![false; cols * rows];
    seen[rows / 2 * cols + cols / 2] = true;
    ui.compose_seen_map("seen_a", "lbl_mapm01aa", &seen, cols, rows);
    seen[rows / 2 * cols + cols / 2 + 1] = true;
    seen[3] = true;
    ui.compose_seen_map("seen_a", "lbl_mapm01aa", &seen, cols, rows);
    let step = ui.texture("seen_a").unwrap().rgba.clone();
    // Another name is made whole.
    ui.compose_seen_map("seen_b", "lbl_mapm01aa", &seen, cols, rows);
    let whole = ui.texture("seen_b").unwrap().rgba.clone();
    assert!(step == whole, "cell by cell is not the same as all at once");
}
