//! A headless fight in the Endar Spire: the player with a blaster pistol against a Sith soldier, with the game's
//! own AI scripts (heartbeat, notice, attacked, death) running. Prints the events and the attack log.

use kotor_archive::Install;
use kotor_game::{Kind, World};

fn main() {
    let inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    let foes: Vec<_> = w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Creature && o.template.starts_with("n_sithsoldier"))
        .map(|o| {
            (
                o.id,
                o.template.clone(),
                o.position,
                o.hp,
                o.abilities,
                o.perception,
            )
        })
        .collect();
    for f in &foes {
        println!("foe {f:?}");
    }
    let target = foes[0].0;
    let pos = foes[0].2;
    let gun = w.spawn_item("g_w_blstrpstl001");
    let pc = w.pc;
    w.object_mut(pc).unwrap().equipped.insert(4, gun);
    w.teleport(pc, [pos[0] - 6.0, pos[1], pos[2]]);
    w.events.clear();
    w.attack(pc, target);
    for _ in 0..1200 {
        w.step(0.05);
        if w.object(target).unwrap().dead || w.object(pc).unwrap().dead {
            break;
        }
    }
    for e in &w.events {
        println!("EVENT {e}");
    }
    for a in &w.combat.log {
        println!("{a:?}");
    }
    println!("stubbed {:?}", w.stubbed);
    println!("constants {:?}", w.constant_defaults);
    println!("errors {:?}", w.errors);
    println!(
        "pc hp {} soldier hp {} dead {}",
        w.object(pc).unwrap().hp,
        w.object(target).unwrap().hp,
        w.object(target).unwrap().dead
    );
}
