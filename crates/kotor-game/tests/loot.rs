use kotor_archive::Install;
use kotor_game::{Kind, ObjId, World};

fn world_in(module: &str) -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.interactive_dialogue = true;
    w.load_module(module).unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    Some(w)
}

/// Stand beside an object and use it.
fn use_it(w: &mut World, id: ObjId) -> bool {
    let at = w.object(id).unwrap().position;
    let pc = w.pc;
    w.teleport(pc, [at[0] + 0.5, at[1], at[2]]);
    w.click(id)
}

/// A placed container whose `OnOpen` is this script.
fn with_on_open(w: &World, script: &str) -> ObjId {
    w.objects
        .values()
        .find(|o| {
            o.kind == Kind::Placeable
                && o.scripts
                    .get("OnOpen")
                    .is_some_and(|s| s.eq_ignore_ascii_case(script))
        })
        .unwrap_or_else(|| panic!("nothing placed with {script}"))
        .id
}

#[test]
fn a_footlocker_gives_its_item_list_once() {
    let Some(mut w) = world_in("end_m01aa") else {
        return;
    };
    let pc = w.pc;
    let locker = w.by_tag("end_locker01")[0];
    assert!(use_it(&mut w, locker));
    assert_eq!(w.opened, Some(locker));
    let n = w.container_items().len();
    assert!(n > 0, "the stock file lists items");
    w.close_container();
    assert_eq!(w.opened, None);
    assert!(use_it(&mut w, locker));
    assert_eq!(w.container_items().len(), n, "not filled a second time");

    // Taken things stay taken, through a save too.
    let first = w.container_items()[0].0;
    w.take_item(first);
    assert_eq!(w.container_items().len(), n - 1);
    assert!(!w.carried(pc).is_empty() || w.gold > 0);
    let bytes = w.save_bytes("Looted").unwrap();
    let mut b = world_in("end_m01aa").unwrap();
    b.load_save(&bytes).unwrap();
    assert_eq!(w.state_digest(), b.state_digest());
    let locker = b.by_tag("end_locker01")[0];
    assert!(use_it(&mut b, locker));
    assert_eq!(b.container_items().len(), n - 1, "not filled again on load");
}

#[test]
fn the_stock_treasure_scripts_fill_a_container_from_their_tables() {
    let Some(mut w) = world_in("kas_m22aa") else {
        return;
    };
    let chest = with_on_open(&w, "k_plc_tresmillow");
    assert!(use_it(&mut w, chest));
    assert_eq!(w.opened, Some(chest));
    let made = w.container_items();
    assert!(!made.is_empty(), "the script made treasure");
    for (id, name) in &made {
        let o = w.object(*id).unwrap();
        assert_eq!(o.kind, Kind::Item);
        assert!(!name.is_empty() && o.base_item > 0, "{name} {}", o.template);
    }
    assert!(
        w.errors.is_empty() && !w.stubbed.contains_key("CreateItemOnObject"),
        "{:?}",
        w.errors
    );
    // It runs at each opening, and the script itself decides whether to make more.
    let n = made.len();
    w.close_container();
    assert!(use_it(&mut w, chest));
    let again = w.container_items().len();
    assert_eq!(again, n, "the script keeps to one filling");

    // Everything comes out, and credits become credits.
    let before = w.gold;
    let stacks: i32 = made
        .iter()
        .filter(|(id, _)| w.object(*id).unwrap().template.contains("credits"))
        .map(|(id, _)| w.object(*id).unwrap().stack)
        .sum();
    for (id, _) in w.container_items() {
        w.take_item(id);
    }
    assert!(w.container_items().is_empty());
    assert_eq!(w.gold, before + stacks);
}

#[test]
fn every_table_makes_real_items() {
    // The other treasure scripts, as many as are placed in this module.
    let Some(mut w) = world_in("kas_m22ab") else {
        return;
    };
    for script in ["k_plc_trescorlow", "k_plc_trescormid", "k_plc_tresdrdlow"] {
        let chest = with_on_open(&w, script);
        // The droid one is locked, and locks are not built.
        w.object_mut(chest).unwrap().locked = false;
        assert!(use_it(&mut w, chest));
        let made = w.container_items();
        assert!(!made.is_empty(), "{script}");
        for (id, name) in made {
            let o = w.object(id).unwrap();
            assert!(o.base_item > 0, "{script} made {name} ({})", o.template);
        }
        w.close_container();
    }
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

#[test]
fn a_locked_container_stays_shut_and_scripts_hear_of_the_rest() {
    let Some(mut w) = world_in("end_m01aa") else {
        return;
    };
    let pc = w.pc;
    let locker = w.by_tag("end_locker01")[0];
    w.object_mut(locker).unwrap().locked = true;
    use_it(&mut w, locker);
    assert_eq!(w.opened, None);
    assert!(w.events.iter().any(|e| e.ends_with("is locked")));
    w.object_mut(locker).unwrap().locked = false;

    // The stock script that removes a container once something is taken.
    let script = "k_pdan_destself";
    let o = w.object_mut(locker).unwrap();
    o.scripts.insert("OnInvDisturbed".into(), script.into());
    assert!(use_it(&mut w, locker));
    let first = w.container_items()[0].0;
    w.take_item(first);
    assert_eq!(w.last_disturbed_by, pc);
    assert_eq!(w.disturbed_item, first);
    for name in ["GetLastDisturbed", "GetInventoryDisturbItem"] {
        assert!(!w.stubbed.contains_key(name), "{name}");
    }
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

/// A creature from the BIFs that carries items its template marks droppable.
fn calo(w: &mut World) -> ObjId {
    let at = w.object(w.pc).unwrap().position;
    w.spawn_creature("n_calonord", [at[0] + 1.0, at[1], at[2]])
}

#[test]
fn a_creature_leaves_what_is_marked_droppable_on_its_body() {
    let Some(mut w) = world_in("end_m01aa") else {
        return;
    };
    let pc = w.pc;
    let foe = calo(&mut w);
    let carried = w.object(foe).unwrap().inventory.len();
    assert!(carried >= 2, "its template's ItemList is in its bag");
    assert!(w.nearest_usable(3.0).is_some());
    assert!(!w.lootable(foe), "it is alive");
    let before = w.object(foe).unwrap().hp;
    assert!(w.hurt(pc, foe, before + 1));
    assert!(w.object(foe).unwrap().dead);
    assert!(w.lootable(foe), "it left something");
    assert_eq!(w.object(foe).unwrap().name, "Remains");
    let items = w.object(foe).unwrap().inventory.clone();
    assert!(!items.is_empty());
    for i in &items {
        assert!(w.object(*i).unwrap().droppable);
    }
    let names: Vec<String> = items
        .iter()
        .map(|i| w.object(*i).unwrap().template.clone())
        .collect();
    assert!(names.iter().any(|n| n == "g_w_hvyblstr03"), "{names:?}");

    // A click opens the body, and what is taken comes to the player.
    let at = w.object(foe).unwrap().position;
    let (o, d) = ([at[0], at[1], at[2] + 3.0], [0.0, 0.0, -1.0]);
    assert_eq!(w.pick(o, d), Some(foe), "the body can be clicked");
    assert!(w.click(foe));
    assert_eq!(w.opened, Some(foe));
    let taken = w.container_items();
    assert_eq!(taken.len(), items.len());
    let bytes = w.save_bytes("Body").unwrap();
    for (id, _) in taken {
        w.take_item(id);
    }
    assert!(!w.carried(pc).is_empty());
    assert!(!w.lootable(foe), "an empty body is scenery again");
    assert_eq!(w.pick(o, d), None);
    w.close_container();
    assert!(!w.click(foe));

    // What was on it is still on it after a load.
    let mut b = world_in("end_m01aa").unwrap();
    b.load_save(&bytes).unwrap();
    let body = b
        .objects
        .values()
        .find(|o| o.template == "n_calonord")
        .unwrap()
        .id;
    assert!(b.lootable(body));
    assert_eq!(b.object(body).unwrap().inventory.len(), items.len());
}

#[test]
fn what_is_not_marked_goes_with_the_creature() {
    let Some(mut w) = world_in("end_m01aa") else {
        return;
    };
    let pc = w.pc;
    // A Sith soldier's rifle or blade is not marked, and it carries nothing else.
    let soldier = w
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .unwrap()
        .id;
    let hp = w.object(soldier).unwrap().hp;
    assert!(w.hurt(pc, soldier, hp + 1));
    assert!(!w.lootable(soldier));
    assert!(w.object(soldier).unwrap().inventory.is_empty());
    assert!(
        w.object(soldier).unwrap().equipped.contains_key(&4),
        "it still holds its weapon"
    );
    let at = w.object(soldier).unwrap().position;
    assert_eq!(w.pick([at[0], at[1], at[2] + 3.0], [0.0, 0.0, -1.0]), None);
}
