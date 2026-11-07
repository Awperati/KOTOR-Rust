//! Grenades thrown by enemies, and what a trigger holds, through the game's own scripts.

use kotor_archive::Install;
use kotor_game::{Kind, World};

const GRENADE: &str = "g_w_fraggren01";
const STEP: f32 = 0.05;

fn world(module: &str) -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module(module).unwrap();
    Some(w)
}

/// A soldier near the player, set to throw, with `count` grenades. The player has company, as the AI wants a group.
fn grenadier(w: &mut World, count: i32) -> (u32, u32) {
    let pc = w.pc;
    let at = w.object(pc).unwrap().position;
    let soldier = w
        .objects
        .values()
        .find(|o| o.kind == Kind::Creature && !o.dead && w.reputation(o.id, pc) <= 10)
        .map(|o| o.id)
        .expect("an enemy");
    w.teleport(soldier, [at[0] + 8.0, at[1], at[2]]);
    w.object_mut(soldier).unwrap().ai_style = 4;
    let item = w.spawn_item(GRENADE);
    w.carry(soldier, item);
    w.object_mut(item).unwrap().stack = count;
    (soldier, item)
}

#[test]
fn the_game_says_who_throws_and_a_thrower_uses_up_what_it_carries() {
    let Some(mut w) = world("end_m01ab") else {
        return;
    };
    w.enter_area();
    let pc = w.pc;
    let (soldier, item) = grenadier(&mut w, 3);
    assert_eq!(w.carried_power(soldier, 87), Some(item));
    assert_eq!(w.carried_power(pc, 87), None);
    let hp = w.object(pc).unwrap().hp;
    w.object_mut(pc).unwrap().max_hp = 500;
    w.object_mut(pc).unwrap().hp = 500;
    w.attack(pc, soldier);
    w.attack(soldier, pc);
    let mut left = 3;
    for _ in 0..(120.0 / STEP) as usize {
        // The AI will not throw at what is close enough to catch it too: it stays off.
        let at = w.object(pc).unwrap().position;
        w.teleport(soldier, [at[0] + 8.0, at[1], at[2]]);
        // Wherever that is, it can see them.
        w.combat.perceived.insert((soldier, pc));
        w.step(STEP);
        w.object_mut(soldier).unwrap().hp = 50;
        left = w.object(item).map_or(0, |o| o.stack);
        if left < 3 {
            break;
        }
    }
    assert!(left < 3, "{hp} {:?}", w.errors);
    assert!(
        !w.effects_at.is_empty(),
        "the burst is somewhere to be seen"
    );
}

#[test]
fn a_grenade_hurts_all_in_its_circle_and_not_those_outside() {
    let Some(mut w) = world("end_m01aa") else {
        return;
    };
    let pc = w.pc;
    let at = w.object(pc).unwrap().position;
    let soldiers: Vec<u32> = w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Creature && o.template.starts_with("n_sithsoldier"))
        .map(|o| o.id)
        .take(3)
        .collect();
    assert_eq!(soldiers.len(), 3);
    w.teleport(soldiers[0], [at[0] + 10.0, at[1], at[2]]);
    w.teleport(soldiers[1], [at[0] + 12.0, at[1], at[2]]);
    w.teleport(soldiers[2], [at[0] + 40.0, at[1], at[2]]);
    for s in &soldiers {
        let o = w.object_mut(*s).unwrap();
        o.max_hp = 500;
        o.hp = 500;
    }
    let grenade = w.spawn_item(GRENADE);
    w.carry(pc, grenade);
    w.use_item_on(pc, grenade, soldiers[0]).unwrap();
    let hp = |w: &World, id: u32| w.object(id).unwrap().hp;
    assert!(hp(&w, soldiers[0]) < 500, "the one aimed at");
    assert!(hp(&w, soldiers[1]) < 500, "the one beside");
    assert_eq!(hp(&w, soldiers[2]), 500, "the one far off");
    assert!(w.carried(pc).iter().all(|i| *i != grenade), "it is gone");
}

#[test]
fn the_leviathan_s_gas_finds_who_is_in_the_room() {
    let Some(mut w) = world("lev_m40aa") else {
        return;
    };
    let pc = w.pc;
    let room = w.by_tag("lev40_gas04");
    let room = *room.first().expect("the room");
    let middle = {
        let p = &w.object(room).unwrap().polygon;
        let n = p.len() as f32;
        [
            p.iter().map(|q| q[0]).sum::<f32>() / n,
            p.iter().map(|q| q[1]).sum::<f32>() / n,
        ]
    };
    let height = w.object(pc).unwrap().position[2];
    w.teleport(pc, [middle[0], middle[1], height]);
    assert!(!w.is_disabled(pc));
    w.run_script("k_plev_gas04", pc).unwrap();
    assert!(w.is_disabled(pc), "{:?}", w.errors);
}

/// Does a soldier with grenades throw one in `seconds`, when the player stands `metres` away and neither moves?
fn throws_from(metres: f32, seconds: f32) -> Option<bool> {
    let mut w = world("end_m01ab")?;
    w.enter_area();
    w.set_rng_seed(7);
    let pc = w.pc;
    let (soldier, item) = grenadier(&mut w, 5);
    let at = w.object(pc).unwrap().position;
    w.object_mut(pc).unwrap().max_hp = 500;
    w.object_mut(pc).unwrap().hp = 500;
    w.object_mut(soldier).unwrap().min_one_hp = true;
    w.combat.perceived.insert((soldier, pc));
    w.attack(soldier, pc);
    for _ in 0..(seconds / STEP) as usize {
        w.teleport(soldier, [at[0] + metres, at[1], at[2]]);
        w.step(STEP);
        if w.object(item).map_or(0, |o| o.stack) < 5 {
            return Some(true);
        }
    }
    Some(false)
}

#[test]
fn a_soldier_throws_at_a_foe_far_off_and_keeps_its_grenades_for_one_close_up() {
    // The game's own AI script decides; the driver of 7b8bd02 saw soldiers hold 5 grenades for 90 s once they had run up.
    // Over 4 m it throws within a few rounds; at 4 m or less it never does, and fights with its weapon.
    let (Some(far), Some(near)) = (throws_from(8.0, 30.0), throws_from(4.0, 60.0)) else {
        return;
    };
    assert!(far, "8 m: it throws");
    assert!(!near, "4 m: it keeps them");
}
