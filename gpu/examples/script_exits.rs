//! Which objects of a module have a script that starts a module, and what the probe says: `script_exits <module> ...`
use kotor_game::Kind;
use kotor_gpu::stage::Stage;

/// Tag, kind, link and scripts of one object.
type Row = (String, Kind, String, Vec<(String, String)>);

/// Every module of the install, from the `modules` folder, when none is named.
fn all_modules() -> Vec<String> {
    let Some(dir) = kotor_core::install_dir() else {
        return Vec::new();
    };
    let mut names = std::collections::BTreeSet::new();
    if let Ok(read) = std::fs::read_dir(std::path::Path::new(&dir).join("modules")) {
        for e in read.flatten() {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            if let Some(m) = n.strip_suffix(".rim").or_else(|| n.strip_suffix(".mod")) {
                names.insert(m.trim_end_matches("_s").to_string());
            }
        }
    }
    names.into_iter().collect()
}

fn main() {
    let mut mods: Vec<String> = std::env::args().skip(1).collect();
    if mods.is_empty() {
        mods = all_modules();
    }
    let (mut count, mut linked) = (0, 0);
    for m in mods {
        let Ok(mut s) = Stage::open(&m, true) else {
            continue;
        };
        s.update(1.0 / 60.0);
        let w = &mut s.game.world;
        let objs: Vec<Row> = w
            .objects
            .values()
            .filter(|o| matches!(o.kind, Kind::Door | Kind::Trigger | Kind::Placeable))
            .map(|o| {
                (
                    o.tag.clone(),
                    o.kind,
                    o.link_module.clone(),
                    o.scripts
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                )
            })
            .collect();
        for (tag, kind, link, scripts) in objs {
            for (slot, script) in scripts {
                if script.is_empty() {
                    continue;
                }
                let to = w.script_destination(&script, -1);
                if to.is_some() {
                    count += 1;
                    linked += (!link.is_empty()) as usize;
                    println!("{m}	{kind:?}	{tag}	link={link:?}	{slot}={script}	{to:?}");
                }
            }
        }
    }
    eprintln!("{count} scripted exits found, {linked} of them on objects that also have a link");
}
