//! Cast every Force power from `spells.2da` once (cost and range ignored) and report what its impact script did:
//! routines it needed that are not implemented, and the effects it left on the target.

use kotor_archive::Install;
use kotor_game::World;

fn main() {
    let inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    let t = kotor_tables::TwoDa::parse(
        &Install::open(kotor_core::install_dir().unwrap())
            .unwrap()
            .read("spells", kotor_core::ResType::TWODA)
            .unwrap(),
    )
    .unwrap();
    let caster = w.pc;
    let target = w.by_tag("end_trask")[0];
    if let Some(row) = std::env::args().nth(1).and_then(|a| a.parse::<i32>().ok()) {
        w.trace = Some(Vec::new());
        let r = w.cast_spell(caster, row, target, true);
        println!("{r:?}");
        w.step(0.1);
        for t in w.trace.take().unwrap() {
            println!("{t}");
        }
        return;
    }
    let mut ran = 0;
    for row in 0..t.row_count() {
        let script = t.get(row, "impactscript").unwrap_or("").to_string();
        if script.is_empty() || t.int(row, "forcepoints").unwrap_or(0) == 0 {
            continue;
        }
        // Personal powers land on the caster, the rest on the target.
        let target = if w
            .spell_cost_and_range(row as i32)
            .is_some_and(|c| c.1 == 0.0)
        {
            caster
        } else {
            target
        };
        w.object_mut(target).unwrap().effects.clear();
        let hp = w.object(target).unwrap().hp;
        let r = w.cast_spell(caster, row as i32, target, true);
        // Many powers apply their effects from a DelayCommand(0.0, ...), so let the clock run a moment.
        w.step(0.1);
        let o = w.object(target).unwrap();
        let fx: Vec<&str> = o
            .effects
            .iter()
            .filter_map(|h| w.effects.get(*h as usize - 1))
            .map(|e| e.name.as_str())
            .collect();
        println!(
            "{row:3} {:36} {script:18} {r:?} hp {hp}->{} effects {fx:?}",
            t.get(row, "label").unwrap_or(""),
            o.hp
        );
        ran += 1;
        w.object_mut(target).unwrap().hp = hp;
        w.object_mut(target).unwrap().dead = false;
    }
    println!("{ran} powers cast");
    println!("stubbed {:?}", w.stubbed);
    println!("constants {:?}", w.constant_defaults);
    println!("errors {:?}", w.errors);
}
