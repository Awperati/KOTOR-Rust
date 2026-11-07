//! Lists every module-to-module link of the given modules, with the offset that would join them and why not when it
//! cannot. `cargo run --release -p kotor-game --example seamless_survey -- tar_m02aa tar_m02ab ...`
use kotor_archive::Install;
use kotor_game::seamless::{plan_module, Sketch};
use kotor_game::World;
use std::collections::HashMap;

fn sketch(name: &str) -> Result<Sketch, String> {
    let inst =
        Install::open(kotor_core::install_dir().ok_or("no install")?).map_err(|e| e.to_string())?;
    let mut w = World::new(inst).map_err(|e| e.to_string())?;
    w.load_module(name).map_err(|e| e.to_string())?;
    // As the game has it once the player is in: the module's own start-up has run (`KOTOR_SURVEY_ENTER=1`).
    if std::env::var_os("KOTOR_SURVEY_ENTER").is_some() {
        w.enter_area();
        for _ in 0..std::env::var("KOTOR_SURVEY_STEPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
        {
            w.step(1.0 / 60.0);
        }
    }
    Ok(w.sketch())
}

fn main() {
    let mut cache: HashMap<String, Result<Sketch, String>> = HashMap::new();
    for m in std::env::args().skip(1) {
        let from = match sketch(&m) {
            Ok(s) => s,
            Err(e) => {
                println!("| {m} | cannot load: {e} |");
                continue;
            }
        };
        let plans = plan_module(&from, |n| {
            cache
                .entry(n.to_string())
                .or_insert_with(|| sketch(n))
                .clone()
        });
        for (site, r) in plans {
            let at = format!("({:.1}, {:.1})", site.at[0], site.at[1]);
            let out = site
                .out
                .map_or("?".into(), |o| format!("{:.0}", o.to_degrees()));
            match r {
                Ok(p) => println!(
                    "| {m} | {} {:?} | {at} | {out} | {} / {} | {}: shift ({:.1}, {:.1}, {:.1}), turn {:.0} deg | {:.2} m, {:.1} deg |",
                    site.tag,
                    site.kind,
                    site.to_module,
                    site.to_waypoint,
                    if p.portal { "portal" } else { "placed" },
                    p.rigid.shift[0],
                    p.rigid.shift[1],
                    p.rigid.shift[2],
                    p.rigid.yaw.to_degrees(),
                    p.residual.0,
                    p.residual.1.to_degrees()
                ),
                Err(e) => println!(
                    "| {m} | {} {:?} | {at} | {out} | {} / {} | falls back: {e} | |",
                    site.tag, site.kind, site.to_module, site.to_waypoint
                ),
            }
        }
    }
}
