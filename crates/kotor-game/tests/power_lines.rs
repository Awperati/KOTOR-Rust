//! A weaker power under a stronger one of its line: the game's own script puts nothing on, so the player's cast is refused and
//! costs nothing. In Classic as in Modern combat.
use kotor_archive::Install;
use kotor_game::World;

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

#[test]
fn a_weaker_power_under_a_stronger_one_is_refused_and_costs_nothing() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    {
        let o = w.object_mut(pc).unwrap();
        (o.force, o.max_force) = (1000, 1000);
        // Force Armour, Force Aura, Force Shield, Force Immunity, Resist Force.
        o.spells.extend([17, 18, 24, 20, 41]);
    }
    for (strong, weak) in [(17, 18), (17, 24), (20, 41)] {
        w.cast_spell(pc, strong, pc, false).unwrap();
        w.step(0.2);
        assert!(w.power_active(pc, strong), "{strong}");
        assert_eq!(w.stronger_on(pc, weak), Some(strong));
        let force = w.object(pc).unwrap().force;
        let e = w.cast_spell(pc, weak, pc, false).unwrap_err();
        assert!(e.contains("stronger"), "{e}");
        assert_eq!(w.object(pc).unwrap().force, force, "nothing is paid");
        // A stronger one over a weaker one is not refused, and a script's cast is not either.
        w.end_power(pc, strong);
        w.cast_spell(pc, weak, pc, false).unwrap();
        w.step(0.2);
        assert!(w.power_active(pc, weak), "{weak}");
        assert_eq!(w.stronger_on(pc, strong), None);
        w.end_power(pc, weak);
        w.cast_spell(pc, strong, pc, false).unwrap();
        w.step(0.2);
        assert!(w.cast_spell(pc, weak, pc, true).is_ok(), "a script's");
        w.step(0.2);
        w.end_power(pc, strong);
        w.end_power(pc, weak);
    }
}
