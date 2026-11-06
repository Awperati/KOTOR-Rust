//! A fighter with a blaster shoots at any distance and does not punch (owner bug: arena duelists).

use kotor_archive::Install;
use kotor_game::{Game, World};

#[test]
fn a_duelist_with_a_blaster_in_its_bag_shoots_point_blank() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.load_module("tar_m02ae").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    let pc = w.pc;
    w.object_mut(pc).unwrap().min_one_hp = true;
    let duelist = w.by_tag("DeadeyeDun021")[0];
    assert!(!w.object(duelist).unwrap().equipped.contains_key(&4));
    let p = w.object(pc).unwrap().position;
    w.teleport(duelist, [p[0] + 1.0, p[1], p[2]]);
    w.attack(duelist, pc);
    let mut g = Game::new(w);
    let mut clips = Vec::new();
    for _ in 0..(8.0 * 60.0) as usize {
        g.step_once();
        if let Some(c) = g.clip(duelist) {
            clips.push(c.name);
        }
    }
    let w = &mut g.world;
    let hand = w.object(duelist).unwrap().equipped.get(&4).copied();
    assert!(hand.is_some(), "still empty handed");
    assert!(w.weapon_of(duelist).ranged, "holds a melee weapon");
    assert!(w.combat.log.iter().any(|l| l.attacker == duelist));
    // The fire clips are `b5a1`; the fist's is `g8a1`.
    assert!(clips.iter().any(|c| c.starts_with('b')), "{clips:?}");
    assert!(!clips.contains(&"g8a1"), "punched");
}
