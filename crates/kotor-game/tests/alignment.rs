//! Light and dark, and how a faction feels, through the game's own scripts where it has them.

use kotor_archive::Install;
use kotor_game::{Kind, World};

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

#[test]
fn the_games_scripts_move_the_player_towards_their_side() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    w.object_mut(pc).unwrap().alignment = 50;
    let mut last = 50;
    for script in ["k_act_lightsml", "k_act_lightmed", "k_act_lighthigh"] {
        w.run_script(script, pc).unwrap();
        let now = w.object(pc).unwrap().alignment;
        assert!(now > last, "{script}: {now}");
        last = now;
    }
    for script in ["k_act_darksml", "k_act_darkmed", "k_act_darkhigh"] {
        w.run_script(script, pc).unwrap();
        let now = w.object(pc).unwrap().alignment;
        assert!(now < last, "{script}: {now}");
        last = now;
    }
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

#[test]
fn alignment_stays_inside_its_scale_and_neutral_stops_at_the_middle() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    for script in ["k_act_lighthigh"; 20] {
        w.run_script(script, pc).unwrap();
    }
    assert_eq!(w.object(pc).unwrap().alignment, 100);
    for script in ["k_act_darkhigh"; 40] {
        w.run_script(script, pc).unwrap();
    }
    assert_eq!(w.object(pc).unwrap().alignment, 0);

    // `nwscript.nss`'s own examples.
    let mut neutral = |from: i32, shift: i32| {
        w.object_mut(pc).unwrap().alignment = from;
        w.adjust_alignment(pc, 1, shift);
        w.object(pc).unwrap().alignment
    };
    assert_eq!(neutral(80, 15), 65);
    assert_eq!(neutral(40, 15), 50);
}

#[test]
fn a_power_costs_more_the_further_you_stray_from_its_side() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let (light, dark, either) = sides();
    let base = |w: &mut World, s: i32| w.spell_cost_and_range(s).unwrap().0;
    let (lc, dc, ec) = (
        base(&mut w, light),
        base(&mut w, dark),
        base(&mut w, either),
    );
    let mut cost = |a: i32, s: i32| {
        w.object_mut(pc).unwrap().alignment = a;
        w.power_cost(pc, s).unwrap()
    };
    assert_eq!(cost(50, light), lc);
    assert_eq!(cost(50, dark), dc);
    assert_eq!(cost(100, light), lc - lc / 2);
    assert_eq!(cost(100, dark), dc * 2);
    assert_eq!(cost(0, light), lc * 2);
    assert_eq!(cost(0, dark), dc - dc / 2);
    assert!(cost(70, dark) > dc && cost(70, dark) < dc * 2);
    assert_eq!(cost(100, either), ec);
    assert_eq!(cost(0, either), ec);
}

/// A light power, a dark one and one of neither, from the table.
fn sides() -> (i32, i32, i32) {
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let bytes = inst.read("spells", kotor_core::ResType::TWODA).unwrap();
    let t = kotor_tables::TwoDa::parse(&bytes).unwrap();
    let find = |side: &str| {
        (0..t.row_count())
            .find(|r| {
                t.get(*r, "goodevil")
                    .is_some_and(|v| v.eq_ignore_ascii_case(side))
                    && t.int(*r, "forcepoints").unwrap_or(0) > 0
            })
            .unwrap() as i32
    };
    (find("G"), find("E"), find("-"))
}

#[test]
fn a_faction_can_turn_on_the_player_and_the_party_with_them() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let someone = w
        .objects
        .iter()
        .find(|(id, o)| {
            **id != pc
                && o.kind == Kind::Creature
                && w.reputation(**id, pc) > 10
                && !w.party.contains(id)
        })
        .map(|(id, _)| *id)
        .expect("somebody here is not hostile");
    let friend = w.party.iter().copied().find(|id| *id != pc);
    w.adjust_reputation(pc, someone, -100);
    assert!(w.reputation(someone, pc) <= 10);
    assert!(w.reputation(pc, someone) <= 10, "and it shows both ways");
    if let Some(friend) = friend {
        assert!(w.reputation(someone, friend) <= 10);
    }
    // It comes back with a save.
    let bytes = w.save_bytes("reputation").unwrap();
    let mut again = World::new(Install::open(kotor_core::install_dir().unwrap()).unwrap()).unwrap();
    again.load_save(&bytes).unwrap();
    assert!(again.reputation(someone, again.pc) <= 10);
}
