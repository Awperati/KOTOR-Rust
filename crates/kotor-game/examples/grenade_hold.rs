//! Does a soldier with grenades throw them? A fight in the opening module with a fixed seed, no help after the start:
//! how long until the first throw, over several seeds and distances.
//!
//!   cargo run --release -p kotor-game --example grenade_hold -- [module]
use kotor_archive::Install;
use kotor_game::{Kind, World};

fn main() {
    let module = std::env::args().nth(1).unwrap_or("end_m01ab".into());
    let dir = kotor_core::install_dir().expect("install");
    for metres in [6.0f32, 10.0] {
        for companion in [false, true] {
            let mut waits = Vec::new();
            for seed in 1..=8u64 {
                let mut w = World::new(Install::open(&dir).unwrap()).unwrap();
                w.load_module(&module).unwrap();
                w.enter_area();
                w.set_rng_seed(seed);
                let pc = w.pc;
                let at = w.object(pc).unwrap().position;
                let soldier = w
                    .objects
                    .values()
                    .find(|o| o.kind == Kind::Creature && !o.dead && w.reputation(o.id, pc) <= 10)
                    .map(|o| o.id)
                    .expect("an enemy");
                w.teleport(soldier, [at[0] + metres, at[1], at[2]]);
                let item = w.spawn_item("g_w_fraggren01");
                w.carry(soldier, item);
                w.object_mut(item).unwrap().stack = 5;
                w.object_mut(soldier).unwrap().ai_style = 4;
                let o = w.object_mut(pc).unwrap();
                (o.hp, o.max_hp) = (500, 500);

                if companion {
                    w.combat.perceived.insert((soldier, pc));
                }
                if companion {
                    w.attack(soldier, pc);
                }
                let mut thrown = None;
                for i in 0..(120.0 / 0.05) as usize {
                    w.step(0.05);
                    if w.object(item).map_or(0, |o| o.stack) < 5 {
                        thrown = Some(i as f32 * 0.05);
                        break;
                    }
                }
                waits.push(thrown);
            }
            println!("{metres} m: {waits:?}");
        }
    }
}
