//! Weapon Focus adds 1 to the attack roll with the kind of weapon it names, and with no other.

use kotor_archive::Install;
use kotor_game::World;

fn world() -> Option<World> {
    let mut base = Install::open(kotor_core::install_dir()?).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    Some(w)
}

#[test]
fn weapon_focus_adds_one_with_its_own_kind_of_weapon_only() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // (template, the feat that covers it): `feat.2da` 32 blaster, 33 blaster rifle, 35 heavy weapons, 36 lightsaber,
    // 37 melee weapons.
    let kinds = [
        ("g_w_blstrpstl001", 32),
        ("g_w_blstrrfl001", 33),
        ("g_w_rptnblstr01", 35),
        ("g_w_lghtsbr01", 36),
        ("g_w_lngswrd01", 37),
    ];
    for (template, feat) in kinds {
        let item = w.spawn_item(template);
        let o = w.object_mut(pc).unwrap();
        o.equipped.insert(4, item);
        o.feats.retain(|f| !(32..=37).contains(f));
        let plain = w.attack_bonus_of(pc, 4);
        w.object_mut(pc).unwrap().feats.insert(feat);
        assert_eq!(
            w.attack_bonus_of(pc, 4),
            plain + 1,
            "{template} with feat {feat}"
        );
        // Any other Weapon Focus feat does nothing for it.
        for other in [32, 33, 35, 36, 37].into_iter().filter(|f| *f != feat) {
            let o = w.object_mut(pc).unwrap();
            o.feats.retain(|f| !(32..=37).contains(f));
            o.feats.insert(other);
            assert_eq!(
                w.attack_bonus_of(pc, 4),
                plain,
                "{template} with feat {other}"
            );
        }
    }
    // Bare hands have no focus.
    let o = w.object_mut(pc).unwrap();
    o.equipped.remove(&4);
    o.feats.extend([32, 33, 35, 36, 37]);
    let with = w.attack_bonus_of(pc, 4);
    w.object_mut(pc).unwrap().feats.clear();
    assert_eq!(with, w.attack_bonus_of(pc, 4));
}
