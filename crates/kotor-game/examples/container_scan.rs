//! Open every container of every module the way a click does, and say what each holds: bug 12, "no items display within the
//! footlocker panel".
//!
//!   cargo run --release -p kotor-game --example container_scan
use kotor_archive::Install;
use kotor_game::{Kind, ObjectLabel, World};
use std::collections::BTreeMap;

fn main() {
    let dir = kotor_core::install_dir().expect("the game");
    let mut names: Vec<String> = std::fs::read_dir(dir.join("modules"))
        .unwrap()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.to_ascii_lowercase().ends_with(".rim") && !n.ends_with("_s.rim"))
        .map(|n| n[..n.len() - 4].to_ascii_lowercase())
        .collect();
    names.sort();
    let (mut modules, mut opened, mut with_items, mut unnamed) = (0, 0, 0, 0);
    let mut empty: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for module in names {
        let mut w = World::new(Install::open(&dir).unwrap()).unwrap();
        w.interactive_dialogue = true;
        if w.load_module(&module).is_err() {
            println!("{module}: does not load");
            continue;
        }
        modules += 1;
        // Treasure scripts are heartbeats: they fill their container the first time, as the game has it.
        for _ in 0..(8.0 * 20.0) as i32 {
            w.step(0.05);
        }
        let pc = w.pc;
        let ids: Vec<_> = w
            .objects
            .values()
            .filter(|o| o.kind == Kind::Placeable)
            .map(|o| (o.id, o.template.clone(), o.tag.clone()))
            .collect();
        for (id, template, tag) in ids {
            // What a label asks is what is wanted here: does the template have an inventory.
            let named = w.object(id).unwrap().name.clone();
            w.object_mut(id).unwrap().name = "x".to_string();
            w.object_mut(id).unwrap().useable = true;
            let has = w.object_label(id) == Some(ObjectLabel::Loot);
            w.object_mut(id).unwrap().name = named;
            if !has {
                continue;
            }
            w.object_mut(id).unwrap().locked = false;
            w.object_mut(id).unwrap().useable = true;
            let at = w.object(id).unwrap().position;
            w.teleport(pc, [at[0] + 0.5, at[1], at[2]]);
            if !w.click(id) {
                println!("{module} {tag}: a click does nothing");
            }
            if w.opened != Some(id) {
                println!("{module} {tag}: not opened by the click");
                continue;
            }
            opened += 1;
            let items = w.container_items();
            unnamed += items.iter().filter(|i| i.1.trim().is_empty()).count();
            if items.is_empty() {
                let script = w.object(id).unwrap().scripts.get("OnOpen").cloned();
                empty
                    .entry(format!("{template} OnOpen={script:?}"))
                    .or_default()
                    .push(format!("{module}/{tag}"));
            } else {
                with_items += 1;
            }
            w.close_container();
        }
    }
    println!("{modules} modules, {opened} containers opened, {with_items} with items, {unnamed} items with no name");
    println!("empty on opening, by template:");
    for (k, v) in &empty {
        println!(
            "  {k}: {} {}",
            v.len(),
            v.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
        );
    }
}
