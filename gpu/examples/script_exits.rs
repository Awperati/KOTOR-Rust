//! Which objects of a module have a script that starts a module, and what the probe says: `script_exits <module> ...`
use kotor_game::Kind;
use kotor_gpu::stage::Stage;

/// Tag, kind, link and scripts of one object.
type Row = (String, Kind, String, Vec<(String, String)>);

fn main() {
    for m in std::env::args().skip(1) {
        let mut s = Stage::open(&m, true).expect("module");
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
                if to.is_some() || tag.contains("apt") || tag.contains("goebon") {
                    println!("{m} {kind:?} {tag} link={link:?} {slot}={script} -> {to:?}");
                }
            }
        }
    }
}
