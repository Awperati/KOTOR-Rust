//! How the game's own data defines loot and item icons: placeable and creature templates with items or treasure
//! scripts, over the BIFs and every module, and which item templates have which icon. Prints counts and a few names.
//!
//!   cargo run --release -p kotor-game --example loot_census
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_gff::{Gff, Struct, Value};
use kotor_tables::TwoDa;
use std::collections::BTreeMap;

const UTC: ResType = ResType(2027);
const UTP: ResType = ResType(2044);
const UTI: ResType = ResType(2025);

#[derive(Default)]
struct Tally {
    placeables: usize,
    with_inventory: usize,
    with_items: usize,
    locked: usize,
    on_open: BTreeMap<String, usize>,
    other: BTreeMap<String, usize>,
    creatures: usize,
    with_list: usize,
    list_items: usize,
    list_droppable: usize,
    equip_items: usize,
    equip_droppable: usize,
    bodybag: BTreeMap<i64, usize>,
    droppers: Vec<String>,
}

impl Tally {
    fn placeable(&mut self, r: &Struct) {
        self.placeables += 1;
        if !r.int("HasInventory").is_some_and(|h| h != 0) {
            return;
        }
        self.with_inventory += 1;
        self.locked += r.int("Locked").is_some_and(|l| l != 0) as usize;
        self.with_items += r.list("ItemList").is_some_and(|l| !l.is_empty()) as usize;
        for f in ["OnOpen", "OnUsed", "OnInvDisturbed", "OnClosed"] {
            if let Some(Value::ResRef(s)) = r.get(f) {
                if !s.is_empty() {
                    let to = if f == "OnOpen" {
                        &mut self.on_open
                    } else {
                        &mut self.other
                    };
                    *to.entry(format!("{f}:{}", s.to_ascii_lowercase()))
                        .or_default() += 1;
                }
            }
        }
    }

    fn creature(&mut self, r: &Struct, name: &str) {
        self.creatures += 1;
        *self
            .bodybag
            .entry(r.int("BodyBag").unwrap_or(0))
            .or_default() += 1;
        let drop = |i: &Struct| i.int("Dropable").is_some_and(|d| d != 0);
        let items = r.list("ItemList").unwrap_or_default();
        self.with_list += !items.is_empty() as usize;
        self.list_items += items.len();
        let dropped = items.iter().filter(|i| drop(i)).count();
        self.list_droppable += dropped;
        let equip = r.list("Equip_ItemList").unwrap_or_default();
        self.equip_items += equip.len();
        self.equip_droppable += equip.iter().filter(|i| drop(i)).count();
        if dropped > 0 && self.droppers.len() < 6 {
            self.droppers.push(format!(
                "{name}: {:?}",
                items
                    .iter()
                    .filter(|i| drop(i))
                    .map(|i| i.str("InventoryRes").unwrap_or(""))
                    .collect::<Vec<_>>()
            ));
        }
    }
}

fn top(m: &BTreeMap<String, usize>) -> Vec<(usize, String)> {
    let mut v: Vec<_> = m.iter().map(|(k, n)| (*n, k.clone())).collect();
    v.sort();
    v.reverse();
    v.truncate(40);
    v
}

fn main() {
    let dir = kotor_core::install_dir().expect("no install");
    let mut base = Install::open(&dir).unwrap();
    base.load_texture_packs().unwrap();
    let names: Vec<(String, ResType)> = base
        .key()
        .entries
        .iter()
        .map(|e| (e.resref.as_str().to_string(), e.restype))
        .collect();
    let like = |p: &str, t: ResType| -> Vec<String> {
        names
            .iter()
            .filter(|(n, r)| *r == t && n.to_ascii_lowercase().contains(p))
            .map(|(n, _)| n.clone())
            .collect()
    };
    println!("scripts *tres*: {:?}", like("tres", ResType::NCS));
    println!("sources *treas*: {:?}", like("treas", ResType::NSS));
    println!("2DA *item*: {:?}", like("item", ResType::TWODA));
    println!("2DA *treas*: {:?}", like("treas", ResType::TWODA));

    let mut bif = Tally::default();
    for (n, t) in &names {
        if *t == UTP {
            if let Some(g) = base.read(n, UTP).ok().and_then(|b| Gff::parse(&b).ok()) {
                bif.placeable(&g.root);
            }
        } else if *t == UTC {
            if let Some(g) = base.read(n, UTC).ok().and_then(|b| Gff::parse(&b).ok()) {
                bif.creature(&g.root, n);
            }
        }
    }
    let mut modules: Vec<String> = std::fs::read_dir(dir.join("modules"))
        .unwrap()
        .filter_map(|e| {
            let n = e.ok()?.file_name().to_string_lossy().to_ascii_lowercase();
            n.strip_suffix(".rim")
                .filter(|s| !s.ends_with("_s"))
                .map(str::to_string)
        })
        .collect();
    modules.sort();
    let mut area = Tally::default();
    for m in &modules {
        let mut i = Install::open(&dir).unwrap();
        i.set_prefer_rim(true);
        if i.load_module(m).is_err() {
            continue;
        }
        let listed: Vec<(String, ResType)> = i
            .overlay_resources()
            .map(|(n, t, _)| (n.to_string(), t))
            .collect();
        for (n, t) in listed {
            if t == UTP {
                if let Some(g) = i.read(&n, UTP).ok().and_then(|b| Gff::parse(&b).ok()) {
                    area.placeable(&g.root);
                }
            } else if t == UTC {
                if let Some(g) = i.read(&n, UTC).ok().and_then(|b| Gff::parse(&b).ok()) {
                    area.creature(&g.root, &format!("{m}/{n}"));
                }
            }
        }
    }
    for (name, t) in [("BIF", &bif), ("modules", &area)] {
        println!(
            "{name}: {} placeables, {} HasInventory, {} with items, {} locked",
            t.placeables, t.with_inventory, t.with_items, t.locked
        );
        println!(
            "{name}: {} creatures, {} with ItemList, {}/{} droppable, equipped {}/{} droppable",
            t.creatures,
            t.with_list,
            t.list_droppable,
            t.list_items,
            t.equip_droppable,
            t.equip_items
        );
        println!("{name}: BodyBag {:?}", t.bodybag);
        println!("{name}: OnOpen {:?}", top(&t.on_open));
        println!("{name}: other {:?}", top(&t.other));
        println!("{name}: droppers {:#?}", t.droppers);
    }

    // Icons: every item template against the names `i<itemclass>_<number>`.
    let items = TwoDa::parse(&base.read("baseitems", ResType::TWODA).unwrap()).unwrap();
    let has = |i: &mut Install, n: &str| {
        i.read(n, ResType::TPC).is_ok() || i.read(n, ResType::TGA).is_ok()
    };
    let (mut by_model, mut by_tex, mut by_default, mut none) = (0, 0, 0, 0);
    let mut misses = Vec::new();
    let mut armour = Vec::new();
    for (n, t) in &names {
        if *t != UTI {
            continue;
        }
        let Some(g) = base.read(n, UTI).ok().and_then(|b| Gff::parse(&b).ok()) else {
            continue;
        };
        let row = g.root.int("BaseItem").unwrap_or(0) as usize;
        let class = items
            .get(row, "itemclass")
            .unwrap_or("")
            .to_ascii_lowercase();
        let def = items.get(row, "defaulticon").unwrap_or("").to_string();
        let m = g.root.int("ModelVariation").unwrap_or(0);
        let tv = g.root.int("TextureVar").unwrap_or(0);
        let a = format!("i{class}_{m:03}");
        let b = format!("i{class}_{tv:03}");
        if class.starts_with("a_") && armour.len() < 12 {
            armour.push(format!("{n}: {class} model {m} tex {tv} default {def}"));
        }
        if m > 0 && has(&mut base, &a) {
            by_model += 1;
        } else if tv > 0 && has(&mut base, &b) {
            by_tex += 1;
        } else if !def.is_empty() && has(&mut base, &def) {
            by_default += 1;
        } else {
            none += 1;
            if misses.len() < 20 {
                misses.push(format!("{n} row {row} class {class} m{m} t{tv} def {def}"));
            }
        }
    }
    println!("icons: by ModelVariation {by_model}, by TextureVar {by_tex}, default {by_default}, none {none}");
    println!("armour e.g. {armour:#?}");
    println!("misses e.g. {misses:#?}");
    for col in ["droidorhuman", "stacking", "reqfeat0", "reqfeat1"] {
        let mut seen = BTreeMap::new();
        for r in 0..items.row_count() {
            *seen
                .entry(items.get(r, col).unwrap_or("").to_string())
                .or_insert(0) += 1;
        }
        println!("baseitems {col}: {seen:?}");
    }
}
