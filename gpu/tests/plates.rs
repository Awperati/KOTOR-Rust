//! Nameplates and labels: who has one, what they say, and what the mouse and the settings do to them.
use kotor_game::{ObjId, ObjectLabel};
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 30.0;
const SIZE: (u32, u32) = (1280, 720);
/// Clear floor, found for `scripts/drive_move.ps1`.
const MODULE: &str = "danm14aa";
const SPOT: [f32; 3] = [165.17, 136.66, 1.85];
const FACING: f32 = 1.767;

/// On open floor, nothing said, with a settings file of its own.
fn stage(name: &str, settings: &str) -> Option<Stage> {
    kotor_core::install_dir()?;
    let mut s = Stage::open(MODULE, true).ok()?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let dir = std::env::temp_dir().join(format!("kotor_rs_plates_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.txt");
    std::fs::write(&file, settings).unwrap();
    s.use_settings_file(file);
    s.place_player(SPOT, FACING);
    for _ in 0..3 {
        s.update(DT);
    }
    Some(s)
}

/// Someone `metres` ahead of the player.
fn ahead(s: &mut Stage, metres: f32, enemy: bool) -> ObjId {
    let at = [
        SPOT[0] + FACING.cos() * metres,
        SPOT[1] + FACING.sin() * metres,
        SPOT[2],
    ];
    let id = s.game.world.spawn_creature("n_calonord", at);
    let o = s.game.world.object_mut(id).unwrap();
    o.name = "Someone".to_string();
    // A party's faction, which likes the player, or the hostile one.
    o.faction = if enemy { 1 } else { 2 };
    id
}

fn plate_of(s: &Stage, id: ObjId) -> Option<(i32, i32)> {
    s.plates_shown()
        .into_iter()
        .find(|(who, _)| *who == id)
        .map(|(_, at)| at)
}

fn pixel(s: &mut Stage, at: (i32, i32)) -> (f32, f32) {
    let wide = s.state_line();
    let wide: f32 = wide
        .split(' ')
        .find_map(|p| p.strip_prefix("width="))
        .unwrap()
        .parse()
        .unwrap();
    (
        at.0 as f32 * SIZE.0 as f32 / wide,
        at.1 as f32 * SIZE.1 as f32 / 480.0,
    )
}

fn pixel_of(s: &mut Stage, id: ObjId) -> (f32, f32) {
    let at = plate_of(s, id).unwrap();
    pixel(s, at)
}

#[test]
fn an_enemy_has_a_plate_and_a_friend_has_one_under_the_cursor() {
    let Some(mut s) = stage("who", "") else {
        return;
    };
    let (foe, friend) = (ahead(&mut s, 6.0, true), ahead(&mut s, 7.0, false));
    s.game.world.object_mut(friend).unwrap().position[0] += 3.0;
    s.update(DT);
    let at = plate_of(&s, foe).expect("the enemy has a plate");
    assert!(plate_of(&s, friend).is_none(), "not without the cursor");
    let drawn = s.plates_drawn();
    assert!(drawn
        .iter()
        .any(|p| p.text.starts_with("Someone") && p.health.is_some()));
    assert!(at.1 > 0);
}

#[test]
fn the_settings_say_who_has_one_and_how_far() {
    let Some(mut s) = stage(
        "settings",
        "nameplates_neutral=true\nnameplates_friendly=true\n",
    ) else {
        return;
    };
    let friend = ahead(&mut s, 6.0, false);
    s.update(DT);
    assert!(plate_of(&s, friend).is_some(), "always, as asked");
    let Some(mut s) = stage("range", "nameplate_range=5\nobject_labels=false\n") else {
        return;
    };
    let foe = ahead(&mut s, 9.0, true);
    s.update(DT);
    assert!(plate_of(&s, foe).is_none(), "out of range");
    let Some(mut s) = stage("off", "nameplates=false\n") else {
        return;
    };
    let foe = ahead(&mut s, 6.0, true);
    s.update(DT);
    assert!(plate_of(&s, foe).is_none());
    s.toggle_nameplates();
    s.update(DT);
    assert!(plate_of(&s, foe).is_some(), "the key turns them on");
}

#[test]
fn a_click_on_a_plate_is_a_click_on_the_body() {
    let Some(mut s) = stage("click", "") else {
        return;
    };
    let foe = ahead(&mut s, 6.0, true);
    s.update(DT);
    let p = pixel_of(&mut s, foe);
    s.hover(p, SIZE);
    s.update(DT);
    // It stays under the cursor as the target, framed, with its health in words.
    let again = pixel_of(&mut s, foe);
    assert!((again.1 - p.1).abs() < 8.0, "{p:?} then {again:?}");
    let framed = s.plates_drawn().iter().find(|p| p.framed).expect("framed");
    assert!(framed.text.ends_with('%'), "{}", framed.text);
    s.click(again, SIZE);
    let pc = s.game.world.pc;
    assert_eq!(s.game.world.combat.target.get(&pc), Some(&foe));
}

#[test]
fn a_hit_leaves_a_chip_that_goes() {
    let Some(mut s) = stage("chip", "") else {
        return;
    };
    let foe = ahead(&mut s, 6.0, true);
    {
        let o = s.game.world.object_mut(foe).unwrap();
        (o.hp, o.max_hp) = (100, 100);
    }
    s.update(DT);
    let pc = s.game.world.pc;
    s.game.world.hurt(pc, foe, 40);
    s.game.world.object_mut(foe).unwrap().hp = 60;
    s.update(DT);
    let health = |s: &Stage| {
        s.plates_drawn()
            .iter()
            .find(|p| p.text.starts_with("Someone"))
            .and_then(|p| p.health)
            .unwrap()
    };
    let (full, chip) = health(&s);
    assert!((full - 0.6).abs() < 0.01, "{full}");
    assert!(chip > 0.9, "the hit still shows: {chip}");
    s.game.world.object_mut(foe).unwrap().faction = 1;
    for _ in 0..60 {
        s.update(DT);
        s.game.world.object_mut(foe).unwrap().hp = 60;
    }
    let (full, chip) = health(&s);
    assert!(
        (chip - full).abs() < 0.01,
        "gone after a moment: {chip} {full}"
    );
}

#[test]
fn plates_stack_and_stay_out_of_menus() {
    let Some(mut s) = stage("stack", "") else {
        return;
    };
    let (a, b) = (ahead(&mut s, 6.0, true), ahead(&mut s, 6.1, true));
    s.update(DT);
    let (pa, pb) = (plate_of(&s, a).unwrap(), plate_of(&s, b).unwrap());
    assert!((pa.1 - pb.1).abs() >= 12, "{pa:?} {pb:?}");
    s.escape();
    assert!(s.menu_open());
    s.update(DT);
    assert!(s.plates_shown().is_empty());
}

#[test]
fn a_label_says_what_a_thing_holds() {
    let Some(mut s) = stage("labels", "") else {
        return;
    };
    let w = &mut s.game.world;
    let ids: Vec<ObjId> = w.objects.keys().copied().collect();
    let labels: Vec<_> = ids.iter().filter_map(|id| w.object_label(*id)).collect();
    assert!(labels.contains(&ObjectLabel::Door) || labels.contains(&ObjectLabel::Loot));
    // A thing that holds something says so, and scenery with no name says nothing.
    let chest = ids
        .iter()
        .copied()
        .find(|id| w.object_label(*id) == Some(ObjectLabel::Usable));
    if let Some(chest) = chest {
        let item = w.spawn_item("g_w_blstrpstl001");
        w.object_mut(chest).unwrap().inventory.push(item);
        assert_eq!(w.object_label(chest), Some(ObjectLabel::Loot));
        w.object_mut(chest).unwrap().name.clear();
        assert_eq!(w.object_label(chest), None);
    }
}
