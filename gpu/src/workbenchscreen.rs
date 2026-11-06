//! The upgrade bench, as the reference video has it at 1:13:05 and 3:42:00: `upgradesel.gui` chooses a kind of thing,
//! `upgradeitems.gui` one of them, and `upgrade.gui` its slots. Nothing changes until Assemble. A script asks for it with
//! `ShowUpgradeScreen`. **Assumed:** a click on a slot lists what goes in it, as the video shows the slots and not the
//! clicks. A lightsaber's own layout in the file, with a bigger figure, is not used. What an upgrade does is in
//! `crates/kotor-game/src/gear.rs`.

use super::{Screen, Stage};
use kotor_game::{BenchKind, ObjId};
use kotor_render::{Camera, Scene};
use kotor_ui::{Hit, MenuView, Row};
use std::collections::BTreeMap;

/// The file's words start on their box's edge. **Assumed:** this far in, by eye.
const WORDS_IN: i32 = 4;
const KINDS: [(&str, &str, BenchKind); 4] = [
    ("BTN_LIGHTSABER", "LBL_LSABER", BenchKind::Lightsaber),
    ("BTN_RANGED", "LBL_RANGED", BenchKind::Ranged),
    ("BTN_MELEE", "LBL_MELEE", BenchKind::Melee),
    ("BTN_ARMOR", "LBL_ARMOR", BenchKind::Armor),
];
/// A kind's slots, as the video has them: the file's buttons and pictures of one row of three and one of four.
const ROW_OF_THREE: [(&str, &str); 3] = [
    ("BTN_UPGRADE31", "LBL_UPGRADE31"),
    ("BTN_UPGRADE32", "LBL_UPGRADE32"),
    ("BTN_UPGRADE33", "LBL_UPGRADE33"),
];
const ROW_OF_FOUR: [(&str, &str); 4] = [
    ("BTN_UPGRADE41", "LBL_UPGRADE41"),
    ("BTN_UPGRADE42", "LBL_UPGRADE42"),
    ("BTN_UPGRADE43", "LBL_UPGRADE43"),
    ("BTN_UPGRADE44", "LBL_UPGRADE44"),
];
const ROW_OF_TWO: [(&str, &str); 2] = [
    ("BTN_UPGRADE42", "LBL_UPGRADE42"),
    ("BTN_UPGRADE43", "LBL_UPGRADE43"),
];
/// `LB_DESC` has no row for words to follow, so the other layout's list holds them.
const NOT_USED: [&str; 4] = ["3D_MODEL_LS", "LBL_DESCBG_LS", "LB_DESC", "LBL_LSSLOTNAME"];
const CRYSTAL_TYPE: i32 = 0;
const STR_NONE: u32 = 363;
const ITEM_HOLE: [f32; 4] = [320.0, 312.0, 249.0, 92.0];
const ITEM_FOV: f32 = 30.0;
/// How long and how fast the item turns in its hole.
const ITEM_TURN_PER_SECOND: f32 = 0.5;

fn slot_tags(kind: BenchKind) -> &'static [(&'static str, &'static str)] {
    match kind {
        BenchKind::Melee => &ROW_OF_THREE,
        BenchKind::Armor => &ROW_OF_TWO,
        _ => &ROW_OF_FOUR,
    }
}

/// The bench as it is being used.
#[derive(Default)]
pub(super) struct Draft {
    kind: Option<BenchKind>,
    item: Option<ObjId>,
    /// The slot on show, and whether what goes in it is listed.
    slot: usize,
    listing: bool,
    /// What Assemble will do to a slot: an upgrade from the bag, or none to empty it.
    changes: BTreeMap<usize, Option<ObjId>>,
}

impl Stage {
    /// A script has asked for the bench.
    pub(in crate::stage) fn take_bench_request(&mut self) {
        if std::mem::take(&mut self.game.world.upgrade_screen) {
            self.open_bench();
        }
    }

    /// The bench, from a script or a test.
    pub fn open_bench(&mut self) {
        self.menus.bench = Draft::default();
        self.menus.stack.push(Screen::BenchKinds);
        self.menus.hover = None;
    }

    fn bench_items_now(&mut self) -> Vec<ObjId> {
        let Some(kind) = self.menus.bench.kind else {
            return Vec::new();
        };
        let pc = self.game.world.pc;
        self.game.world.bench_items(pc, kind)
    }

    /// `upgradesel.gui`: a kind with nothing to upgrade is greyed.
    pub(super) fn bench_kinds_view(&mut self) -> MenuView {
        let mut v = MenuView::new("upgradesel");
        let pc = self.game.world.pc;
        let chosen = self.menus.bench.kind;
        for (button, label, kind) in KINDS {
            if self.game.world.bench_items(pc, kind).is_empty() {
                v.inert(&[button]);
                v.fill(label, format!("{}2", label_picture(label)));
            } else if chosen == Some(kind) {
                v.lit(button);
            }
        }
        if chosen.is_none() {
            v.inert(&["BTN_UPGRADEITEMS"]);
        }
        v
    }

    /// `upgradeitems.gui`.
    pub(super) fn bench_items_view(&mut self) -> MenuView {
        let mut v = MenuView::new("upgradeitems");
        let items = self.bench_items_now();
        let selected = self.menus.selected;
        let rows: Vec<Row> = items
            .iter()
            .enumerate()
            .filter_map(|(n, i)| {
                let view = self.game.world.item_view(*i)?;
                let mut row = self.item_row(&view, false);
                row.selected = selected == Some(n);
                Some(row)
            })
            .collect();
        v.lists
            .insert("LB_ITEMS".to_string(), (rows, self.menus.first));
        let under = match self.menus.hover {
            Some(Hit::Row(_, n)) => Some(n),
            _ => None,
        };
        let about = under
            .or(selected)
            .and_then(|n| items.get(n))
            .and_then(|i| self.game.world.item_view(*i));
        v.text(
            "LB_DESCRIPTION",
            about.map_or(String::new(), |a| a.description),
        );
        if selected.is_none() {
            v.inert(&["BTN_UPGRADEITEM"]);
        }
        v
    }

    /// `upgrade.gui`: the slots, with what is in them or will be.
    pub(super) fn bench_view(&mut self) -> MenuView {
        let mut v = MenuView::new("upgrade");
        v.hide(&NOT_USED);
        v.holes.push("3D_MODEL".to_string());
        let (Some(kind), Some(item)) = (self.menus.bench.kind, self.menus.bench.item) else {
            return v;
        };
        let (shown_slot, listing) = (self.menus.bench.slot, self.menus.bench.listing);
        let changes = self.menus.bench.changes.clone();
        let tags = slot_tags(kind);
        for row in [&ROW_OF_THREE[..], &ROW_OF_FOUR[..]] {
            for (button, label) in row {
                if !tags.iter().any(|t| t.0 == *button) {
                    v.hide(&[button, label]);
                }
            }
        }
        let installed = self
            .game
            .world
            .object(item)
            .map_or(BTreeMap::new(), |o| o.upgrades.clone());
        let mut names = Vec::new();
        for (slot, (button, label)) in tags.iter().enumerate() {
            let picture = match changes.get(&slot) {
                Some(Some(upgrade)) => {
                    let view = self.game.world.item_view(*upgrade);
                    names.extend(view.as_ref().map(|i| i.name.clone()));
                    view.and_then(|i| i.icons.first().cloned())
                }
                Some(None) => None,
                None => installed.get(&(slot as i32)).and_then(|row| {
                    let (name, picture) = self.game.world.upgrade_look(*row)?;
                    names.push(self.ui.strref(name));
                    Some(picture)
                }),
            };
            if let Some(picture) = picture {
                v.fill(label, picture);
            }
            if slot == shown_slot {
                v.lit(button);
            }
        }
        let pc = self.game.world.pc;
        let slot_type = kind.slots().get(shown_slot).copied().unwrap_or(0);
        let choices = self.game.world.bench_choices(pc, slot_type);
        // Crystals are of many names, and the type has none of its own.
        let name = Some(slot_type)
            .filter(|t| *t != CRYSTAL_TYPE)
            .and_then(|t| self.game.world.upgrade_type_name(t));
        v.text(
            "LBL_SLOTNAME",
            name.map_or(String::new(), |n| self.ui.strref(n)),
        );
        v.text("LBL_UPGRADE_COUNT", choices.len().to_string());
        v.patches
            .entry("LBL_UPGRADES".to_string())
            .or_default()
            .inset = WORDS_IN;
        if listing {
            v.hide(&[
                "LBL_PROPERTY",
                "LBL_SLOTNAME",
                "LBL_UPGRADES",
                "LBL_UPGRADE_COUNT",
            ]);
            let none = self.ui.strref(STR_NONE);
            let mut rows = vec![Row {
                text: none,
                ..Row::default()
            }];
            for upgrade in &choices {
                if let Some(view) = self.game.world.item_view(*upgrade) {
                    rows.push(self.item_row(&view, false));
                }
            }
            v.lists
                .insert("LB_ITEMS".to_string(), (rows, self.menus.first));
        } else {
            v.hide(&["LB_ITEMS"]);
            let properties = self.game.world.item_properties(item);
            let mut lines: Vec<String> = properties.iter().map(|p| self.ui.strref(*p)).collect();
            lines.extend(names);
            v.text("LBL_PROPERTY", lines.join("\n"));
            v.patches
                .entry("LBL_PROPERTY".to_string())
                .or_default()
                .inset = WORDS_IN;
        }
        let about = self.game.world.item_view(item);
        v.text("LB_DESC_LS", about.map_or(String::new(), |a| a.description));
        v
    }

    /// A click on a row of one of the bench's lists.
    pub(super) fn bench_row(&mut self, row: usize) {
        if self.top() != Some(Screen::Bench) {
            self.menus.selected = Some(row);
            return;
        }
        let Some(kind) = self.menus.bench.kind else {
            return;
        };
        let slot = self.menus.bench.slot;
        let pc = self.game.world.pc;
        let slot_type = kind.slots().get(slot).copied().unwrap_or(0);
        let choices = self.game.world.bench_choices(pc, slot_type);
        let chosen = match row {
            0 => None,
            n => match choices.get(n - 1) {
                Some(upgrade) => Some(*upgrade),
                None => return,
            },
        };
        self.menus.bench.changes.insert(slot, chosen);
        self.menus.bench.listing = false;
    }

    fn assemble(&mut self) {
        let Some(item) = self.menus.bench.item else {
            return;
        };
        let pc = self.game.world.pc;
        let changes = std::mem::take(&mut self.menus.bench.changes);
        for (slot, change) in changes {
            match change {
                Some(upgrade) => {
                    self.game
                        .world
                        .install_upgrade(pc, item, slot as i32, upgrade);
                }
                None => self.game.world.remove_upgrade(pc, item, slot as i32),
            }
        }
    }

    /// What a button on the bench's screens does.
    pub(super) fn bench_press(&mut self, screen: Screen, tag: &str) {
        match (screen, tag) {
            (Screen::BenchKinds, "BTN_BACK") | (Screen::BenchItems, "BTN_BACK") => {
                self.menus.stack.pop();
                self.menus.selected = None;
            }
            (Screen::BenchKinds, "BTN_UPGRADEITEMS") => {
                if self.menus.bench.kind.is_some() {
                    self.menus.stack.push(Screen::BenchItems);
                    self.menus.selected = None;
                    self.menus.first = 0;
                }
            }
            (Screen::BenchKinds, _) => {
                if let Some(&(_, _, kind)) = KINDS.iter().find(|k| k.0 == tag) {
                    let pc = self.game.world.pc;
                    if !self.game.world.bench_items(pc, kind).is_empty() {
                        self.menus.bench.kind = Some(kind);
                    }
                }
            }
            (Screen::BenchItems, "BTN_UPGRADEITEM") => {
                let item = self
                    .menus
                    .selected
                    .and_then(|n| self.bench_items_now().get(n).copied());
                if let Some(item) = item {
                    let bench = &mut self.menus.bench;
                    bench.item = Some(item);
                    bench.slot = 0;
                    bench.listing = false;
                    bench.changes.clear();
                    self.menus.bench_turn = 0.0;
                    self.menus.stack.push(Screen::Bench);
                    self.menus.first = 0;
                }
            }
            (Screen::Bench, "BTN_ASSEMBLE") => {
                self.assemble();
                self.menus.stack.pop();
            }
            (Screen::Bench, "BTN_BACK") => {
                self.menus.bench.changes.clear();
                self.menus.stack.pop();
            }
            (Screen::Bench, _) => {
                let Some(kind) = self.menus.bench.kind else {
                    return;
                };
                if let Some(slot) = slot_tags(kind).iter().position(|t| t.0 == tag) {
                    let bench = &mut self.menus.bench;
                    bench.listing = bench.slot != slot || !bench.listing;
                    bench.slot = slot;
                    self.menus.first = 0;
                }
            }
            _ => {}
        }
        self.menus.hover = None;
    }

    /// The item in its hole, turning.
    pub(super) fn bench_scene(&mut self) -> Option<(Scene, Camera)> {
        if self.top()? != Screen::Bench {
            return None;
        }
        let item = self.menus.bench.item?;
        let model = self.item_model(item)?;
        let mut flat = Scene::default();
        flat.add_model(&model, [0.0; 3]);
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        for p in flat.meshes.iter().flat_map(|m| m.positions.iter()) {
            for k in 0..3 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
        let middle = [0, 1, 2].map(|k| (low[k] + high[k]) / 2.0);
        let size = (0..3).map(|k| high[k] - low[k]).fold(0.1, f32::max);
        let (sin, cos) = self.menus.bench_turn.sin_cos();
        let mut scene = Scene::default();
        scene.append_transformed(
            &flat,
            &[
                [cos, -sin, 0.0, -middle[0] * cos + middle[1] * sin],
                [sin, cos, 0.0, -middle[0] * sin - middle[1] * cos],
                [0.0, 0.0, 1.0, -middle[2]],
                [0.0, 0.0, 0.0, 1.0],
            ],
        );
        let [left, top, width, height] = ITEM_HOLE;
        let tan = (ITEM_FOV.to_radians() / 2.0).tan();
        let depth = size / (2.0 * tan * (height / 480.0));
        let aspect = self.ui_width as f32 / 480.0;
        let across = ((left + width / 2.0) / 320.0 - 1.0) * depth * tan * aspect;
        let up = (1.0 - (top + height / 2.0) / 240.0) * depth * tan;
        let camera = Camera {
            eye: [-across, -depth, -up],
            target: [-across, 0.0, -up],
            fov: ITEM_FOV,
        };
        Some((scene, camera))
    }

    pub(in crate::stage) fn bench_tick(&mut self, dt: f32) {
        if self.top() == Some(Screen::Bench) {
            self.menus.bench_turn += dt * ITEM_TURN_PER_SECOND;
        }
    }
}

/// The picture of a kind's tile, with 2 for one that has nothing.
fn label_picture(label: &str) -> &'static str {
    match label {
        "LBL_LSABER" => "lbl_wuplsaber",
        "LBL_RANGED" => "lbl_wuprange",
        "LBL_MELEE" => "lbl_wupmelee",
        _ => "lbl_wuparmor",
    }
}
