//! Leaving the hideout on Taris: the Sith soldier's raid on the neighbours starts as the player steps out.
use kotor_archive::Install;
use kotor_game::World;

/// Out of the hideout's door into `tar_m02aa`, as a new game has it, then a few seconds of play.
fn step_out(interactive: bool) -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut inst = Install::open(dir).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = interactive;
    w.load_module("tar_m02af").unwrap();
    w.enter_area();
    let door = w.by_tag("tar02_aptdoor")[0];
    let (module, to) = {
        let o = w.object(door).unwrap();
        (o.link_module.clone(), o.link_to.clone())
    };
    w.transition(&module, &to).unwrap();
    for _ in 0..40 {
        w.step(0.05);
        if w.in_dialogue() {
            break;
        }
    }
    Some(w)
}

/// The trigger by the door starts its talk with an invisible placeable whose own talk is the raid: the trigger has none.
#[test]
fn stepping_out_of_the_hideout_starts_the_sith_soldiers_raid() {
    let Some(w) = step_out(false) else {
        return;
    };
    let begun = "conversation tar02_preraid begins";
    assert!(
        w.events.iter().any(|e| e.contains(begun)),
        "{:?}",
        w.events
            .iter()
            .filter(|e| e.contains("enters") || e.contains("conversation"))
            .collect::<Vec<_>>()
    );
    // As the player plays it, the talk is held for them at once.
    let Some(w) = step_out(true) else {
        return;
    };
    assert!(w.in_dialogue());
    let (speaker, ..) = w.dialogue_view().expect("a line");
    assert!(!speaker.is_empty());
}

/// A battle droid's gun is part of it ("Blaster Pistol: Null", with no mesh): its body leaves nothing. The soldier's own
/// rifle and armour are still there to take.
#[test]
fn the_droids_built_in_gun_is_no_loot_and_the_soldiers_rifle_is() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("tar_m02aa").unwrap();
    let pc = w.pc;
    let templates = |w: &World, id| -> Vec<String> {
        w.object(id)
            .unwrap()
            .inventory
            .iter()
            .filter_map(|i| w.object(*i))
            .map(|o| o.template.to_ascii_lowercase())
            .collect()
    };
    for tag in ["droid1", "droid2"] {
        let droid = w.by_tag(tag)[0];
        w.hurt(pc, droid, 10_000);
        assert!(w.object(droid).unwrap().dead);
        assert_eq!(templates(&w, droid), Vec::<String>::new(), "{tag}");
        assert!(!w.lootable(droid));
    }
    let soldier = w.by_tag("Trooper1")[0];
    w.hurt(pc, soldier, 10_000);
    let left = templates(&w, soldier);
    assert!(left.contains(&"g_w_blstrrfl001".to_string()), "{left:?}");
    assert!(left.contains(&"g_a_class8001".to_string()), "{left:?}");
}
