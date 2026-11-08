//! Every door and placeable name, as a loaded module has it, with how many of each a player can click: to find the
//! editor's placeholder names ("static", "Door") that a name plate or the target bar would show.
//!
//!   cargo run --release -p kotor-game --example placeholder_names > names.tsv
use kotor_archive::Install;
use kotor_game::{Kind, World};
use std::collections::BTreeMap;

#[derive(Default)]
struct Seen {
    count: usize,
    clickable: usize,
    labelled: usize,
    places: Vec<String>,
}

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
    let mut seen: BTreeMap<(&str, String), Seen> = BTreeMap::new();
    let mut modules = 0;
    for module in names {
        let mut w = World::new(Install::open(&dir).unwrap()).unwrap();
        if w.load_module(&module).is_err() {
            eprintln!("{module}: does not load");
            continue;
        }
        modules += 1;
        let ids: Vec<_> = w
            .objects
            .values()
            .filter(|o| matches!(o.kind, Kind::Door | Kind::Placeable))
            .map(|o| o.id)
            .collect();
        for id in ids {
            let clickable = w.clickable(id);
            let labelled = w.object_label(id).is_some();
            let o = w.object(id).unwrap();
            let kind = if o.kind == Kind::Door {
                "door"
            } else {
                "placeable"
            };
            let s = seen.entry((kind, o.name.clone())).or_default();
            s.count += 1;
            s.clickable += clickable as usize;
            s.labelled += labelled as usize;
            if s.places.len() < 3 {
                s.places.push(format!("{module}/{}/{}", o.tag, o.template));
            }
        }
    }
    eprintln!("{modules} modules, {} names", seen.len());
    println!("kind\tname\tcount\tclickable\tlabelled\tfirst places");
    for ((kind, name), s) in &seen {
        println!(
            "{kind}\t{name:?}\t{}\t{}\t{}\t{}",
            s.count,
            s.clickable,
            s.labelled,
            s.places.join(" ")
        );
    }
}
