//! Fight win rates with and without "No misses": the level-1 player against one and two of a kind, many seeds each.
//!
//!   cargo run --release -p kotor-game --example no_miss_balance -- [trials]
use kotor_archive::Install;
use kotor_game::World;

fn world() -> World {
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    // Nobody else in the area: the fight is the player's and the foes' alone.
    let pc = w.pc;
    let others: Vec<_> = w
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Creature && o.id != pc)
        .map(|o| o.id)
        .collect();
    for id in others {
        w.objects.remove(&id);
    }
    w
}

/// (player won, seconds, player hp lost) for one fight.
fn duel(foes: &[&str], level: i32, no_misses: bool, seed: u64) -> (bool, f32, i32) {
    let mut w2 = world();
    w2.no_misses = no_misses;
    w2.seed_dice(seed);
    let pc = w2.pc;
    w2.object_mut(pc).unwrap().level = level;
    let p = w2.object(pc).unwrap().position;
    let mut ids = vec![];
    for (i, t) in foes.iter().enumerate() {
        let id = w2.spawn_creature(t, [p[0] + 3.0 + i as f32, p[1], p[2]]);
        w2.object_mut(id).unwrap().faction = 1;
        ids.push(id);
    }
    let hp0 = w2.object(pc).unwrap().hp;
    for id in &ids {
        w2.attack(*id, pc);
    }
    w2.attack(pc, ids[0]);
    let mut t = 0.0;
    while t < 90.0 {
        w2.step(0.05);
        t += 0.05;
        let alive = ids
            .iter()
            .filter(|i| w2.object(**i).is_some_and(|o| !o.dead))
            .count();
        if alive == 0 || w2.object(pc).unwrap().dead || w2.object(pc).unwrap().hp <= 0 {
            break;
        }
        // The player moves on to the next one.
        if let Some(n) = ids.iter().find(|i| w2.object(**i).is_some_and(|o| !o.dead)) {
            if w2.combat.target.get(&pc) != Some(n) {
                w2.attack(pc, *n);
            }
        }
    }
    let alive = ids
        .iter()
        .filter(|i| w2.object(**i).is_some_and(|o| !o.dead))
        .count();
    let hp = w2.object(pc).unwrap().hp;
    (alive == 0 && hp > 0, t, hp0 - hp.max(0))
}

fn main() {
    let trials: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(40);
    let cases: [(&str, Vec<&str>, i32); 4] = [
        ("1 Sith soldier, level 1", vec!["n_sithsoldier003"], 1),
        (
            "2 Sith soldiers, level 1",
            vec!["n_sithsoldier003", "n_sithsoldier003"],
            1,
        ),
        ("1 Sith soldier, level 5", vec!["n_sithsoldier003"], 5),
        (
            "2 Sith soldiers, level 5",
            vec!["n_sithsoldier003", "n_sithsoldier003"],
            5,
        ),
    ];
    println!(
        "{:<28} {:>8} {:>8} {:>9} {:>9} {:>9} {:>9}",
        "fight", "win off", "win on", "secs off", "secs on", "hp off", "hp on"
    );
    for (name, foes, level) in cases {
        let mut row = [(0u64, 0.0f32, 0i64); 2];
        for (k, on) in [false, true].into_iter().enumerate() {
            for seed in 1..=trials {
                let (won, secs, lost) = duel(&foes, level, on, seed);
                row[k].0 += won as u64;
                row[k].1 += secs;
                row[k].2 += lost as i64;
            }
        }
        let n = trials as f32;
        println!(
            "{:<28} {:>7.0}% {:>7.0}% {:>9.1} {:>9.1} {:>9.1} {:>9.1}",
            name,
            row[0].0 as f32 * 100.0 / n,
            row[1].0 as f32 * 100.0 / n,
            row[0].1 / n,
            row[1].1 / n,
            row[0].2 as f32 / n,
            row[1].2 as f32 / n
        );
    }
}
