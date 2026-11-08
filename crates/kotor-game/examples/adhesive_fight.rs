//! `drive_fight` ends with the player throwing an adhesive grenade at a Sith soldier and then attacking: does the player attack?
//!
//!   cargo run --release -p kotor-game --example adhesive_fight
use kotor_archive::Install;
use kotor_game::World;

fn main() {
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = true;
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    let pc = w.pc;
    let foe = w
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
        .unwrap();
    let p = w.object(pc).unwrap().position;
    w.teleport(foe, [p[0] + 4.0, p[1], p[2]]);
    w.attack(foe, pc);
    w.attack(pc, foe);
    if std::env::args().nth(1).as_deref() == Some("push") {
        w.object_mut(pc).unwrap().force = 100;
        w.cast_spell(pc, kotor_game::FORCE_PUSH, foe, true).unwrap();
    }
    let g = w.spawn_item("g_w_adhsvgren001");
    w.carry(pc, g);
    w.use_item_on(pc, g, foe).unwrap();
    for i in 0..(30.0 / 0.05) as usize {
        w.step(0.05);
        if i % 20 == 0 {
            let f = w.object(foe).unwrap();
            println!(
                "{:.1}s foe at {:?} pc at {:?} foe hp {} disabled {} pc attacks {} target {:?} errors {:?}",
                w.time,
                f.position.map(|v| (v * 10.0).round() / 10.0),
                w.object(pc).unwrap().position.map(|v| (v * 10.0).round() / 10.0),
                f.hp,
                w.is_disabled(foe),
                w.combat.log.iter().filter(|l| l.attacker == pc).count(),
                w.combat.target.get(&pc),
                w.errors.last()
            );
        }
    }
}
