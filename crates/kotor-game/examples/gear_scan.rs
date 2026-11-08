//! Lists the items whose name has a word in it (default "Sith") with their base item and the item properties in their files,
//! and says which of those properties the gear code reads. Armour and items that carry a property it does not read are
//! the ones that do less in play than their description says.
//!
//!   cargo run --release -p kotor-game --example gear_scan -- [word]
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::World;
use kotor_gff::Gff;
use kotor_tables::TwoDa;

/// `itempropdef.2da` rows that `gear.rs` reads (plus 10, cast spell, read by `items.rs`).
const READ: [i64; 26] = [
    0, 1, 5, 8, 10, 11, 12, 13, 15, 19, 20, 21, 26, 27, 28, 33, 34, 36, 38, 41, 43, 44, 45, 49, 57,
    29,
];

fn main() {
    let word = std::env::args().nth(1).unwrap_or_else(|| "Sith".into());
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let w = World::new(Install::open(kotor_core::install_dir().unwrap()).unwrap()).unwrap();
    let mut two = |n: &str| TwoDa::parse(&inst.read(n, ResType::TWODA).unwrap()).unwrap();
    let (props, base) = (two("itempropdef"), two("baseitems"));
    if word == "--columns" {
        for c in [
            "maxdex",
            "dexbonus",
            "maxdexbonus",
            "armorcheckpen",
            "arcanespellfailure",
            "damageflags",
            "baseac",
            "acenchantment",
        ] {
            println!("baseitems {c}: {:?}", base.column_index(c));
        }
        for r in 0..base.row_count() {
            if base.int(r, "baseac").unwrap_or(0) > 0 {
                println!(
                    "{r} {} baseac {} dexbonus {:?} armorcheckpen {:?}",
                    base.get(r, "label").unwrap_or("?"),
                    base.int(r, "baseac").unwrap_or(0),
                    base.int(r, "dexbonus"),
                    base.int(r, "armorcheckpen")
                );
            }
        }
        return;
    }
    let names = inst.list(ResType(2025));
    for name in names {
        let Ok(b) = inst.read(&name, ResType(2025)) else {
            continue;
        };
        let Ok(g) = Gff::parse(&b) else { continue };
        let Some(label) = g.root.loc("LocalizedName").map(|l| w.text(l)) else {
            continue;
        };
        if !label.to_lowercase().contains(&word.to_lowercase()) {
            continue;
        }
        let bi = g.root.int("BaseItem").unwrap_or(0);
        let ac = base.int(bi as usize, "baseac").unwrap_or(0);
        let kind = base.get(bi as usize, "label").unwrap_or("?");
        let mut shown = Vec::new();
        for p in g.root.list("PropertiesList").unwrap_or_default() {
            let n = p.int("PropertyName").unwrap_or(-1);
            let pn = props.get(n.max(0) as usize, "label").unwrap_or("?");
            let sub = p.int("Subtype").unwrap_or(0);
            let cv = p.int("CostValue").unwrap_or(0);
            let up = p
                .int("UpgradeType")
                .map_or(String::new(), |u| format!(" upgrade{u}"));
            let flag = if READ.contains(&n) { "" } else { " NOT-READ" };
            shown.push(format!("{pn}({n}) sub{sub} cost{cv}{up}{flag}"));
        }
        println!("{name}\t{label}\t{kind} baseac {ac}\t{}", shown.join("; "));
    }
}
