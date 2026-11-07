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

/// What the HUD says is the target.
fn target_name(s: &mut Stage) -> String {
    s.state_line()
        .rsplit(" target=")
        .next()
        .unwrap()
        .trim()
        .to_string()
}

/// The cursor on a foe's plate, and a moment for the slots to come.
fn hover_on(s: &mut Stage, foe: ObjId) {
    let p = pixel_of(s, foe);
    s.hover(p, SIZE);
    s.update(DT);
    s.update(DT);
}

/// Far from any body or button: the top left of the middle of the window.
const NOWHERE: (f32, f32) = (300.0, 60.0);

fn slot_spacing(s: &mut Stage) -> f32 {
    let a = s.hud_pixel("BTN_TARGET0", SIZE).unwrap();
    let b = s.hud_pixel("BTN_TARGET1", SIZE).unwrap();
    b.0 - a.0
}

#[test]
fn the_slots_over_a_target_are_smaller_the_further_off_and_stop_at_each_end() {
    let Some(mut s) = stage("slot_size", "") else {
        return;
    };
    let foe = ahead(&mut s, 7.0, true);
    s.update(DT);
    hover_on(&mut s, foe);
    let at = |m: f32| {
        [
            SPOT[0] + FACING.cos() * m,
            SPOT[1] + FACING.sin() * m,
            SPOT[2],
        ]
    };
    let spacing = |s: &mut Stage, m: f32| {
        s.game.world.object_mut(foe).unwrap().position = at(m);
        s.update(DT);
        let wide = slot_spacing(s);
        assert!(target_name(s).starts_with("Someone"), "{m}");
        wide
    };
    let (close, near, middle, far, farther) = (
        spacing(&mut s, 2.0),
        spacing(&mut s, 4.0),
        spacing(&mut s, 12.0),
        spacing(&mut s, 24.0),
        spacing(&mut s, 40.0),
    );
    assert!(far < middle && middle < near, "{far} {middle} {near}");
    assert_eq!(close, near, "it stops growing");
    assert_eq!(far, farther, "and shrinking");
    let ratio = far / near;
    assert!((0.44..0.52).contains(&ratio), "60 over 125: {ratio}");
}

#[test]
fn a_plate_sits_over_its_own_head_and_an_enemy_s_slots_are_smaller() {
    let Some(mut s) = stage("heads", "") else {
        return;
    };
    // A Wookiee, a man and a Jawa side by side, as far off.
    let side = |s: &mut Stage, id: ObjId, by: f32| {
        let o = s.game.world.object_mut(id).unwrap();
        o.position[0] += FACING.sin() * by;
        o.position[1] -= FACING.cos() * by;
    };
    let tall = ahead(&mut s, 6.0, true);
    s.game.world.object_mut(tall).unwrap().appearance = 51;
    side(&mut s, tall, -2.0);
    let man = ahead(&mut s, 6.0, true);
    let short = ahead(&mut s, 6.0, true);
    s.game.world.object_mut(short).unwrap().appearance = 73;
    side(&mut s, short, 2.0);
    for _ in 0..3 {
        s.update(DT);
    }
    let head = |s: &Stage, id: ObjId| s.measured_head(id).expect("measured");
    assert!(
        head(&s, tall) > head(&s, man) + 0.3,
        "{} {}",
        head(&s, tall),
        head(&s, man)
    );
    assert!(
        head(&s, man) > head(&s, short) + 0.2,
        "{} {}",
        head(&s, man),
        head(&s, short)
    );
    // Higher on the screen over the taller head.
    let y = |s: &Stage, id: ObjId| plate_of(s, id).expect("a plate").1;
    assert!(y(&s, tall) < y(&s, man) && y(&s, man) < y(&s, short));

    // An enemy's attack slots are about two thirds of the size they were: 125 near, 60 far, in fives.
    hover_on(&mut s, man);
    let line = s.state_line();
    let percent: i32 = line
        .split_whitespace()
        .find_map(|w| w.strip_prefix("slotsize="))
        .unwrap()
        .parse()
        .unwrap();
    // Six metres off: 125 less a tenth of the way to 60, times 0.65.
    assert_eq!(percent, 75, "{line}");
}

#[test]
fn the_slots_stay_on_their_target_all_the_way_to_a_click() {
    let Some(mut s) = stage("slots_stay", "") else {
        return;
    };
    let pc = s.game.world.pc;
    let foe = ahead(&mut s, 7.0, true);
    s.update(DT);
    hover_on(&mut s, foe);
    assert!(target_name(&mut s).starts_with("Someone"));
    let first = s.hud_pixel("BTN_TARGET0", SIZE).unwrap();

    // Up through the empty space between its head and the slots.
    let plate = pixel_of(&mut s, foe);
    s.hover((first.0, (first.1 + plate.1) / 2.0), SIZE);
    s.update(DT);
    assert!(target_name(&mut s).starts_with("Someone"), "in the gap");

    // Away from it all, for longer than the grace: nothing else has taken them.
    s.hover(NOWHERE, SIZE);
    for _ in 0..30 {
        s.update(DT);
    }
    assert!(target_name(&mut s).starts_with("Someone"), "off it all");
    assert!(!s.game.world.combat.target.contains_key(&pc));

    // The click on the slot goes for that one, with the cursor nowhere near it.
    let slot = s.hud_pixel("BTN_TARGET0", SIZE).unwrap();
    s.hover(slot, SIZE);
    s.update(DT);
    assert!(target_name(&mut s).starts_with("Someone"), "on the slot");
    s.click(slot, SIZE);
    assert_eq!(s.game.world.combat.target.get(&pc), Some(&foe));
}

#[test]
fn only_another_target_after_the_grace_takes_the_slots() {
    let Some(mut s) = stage("slots_grace", "") else {
        return;
    };
    let (foe, other) = (ahead(&mut s, 7.0, true), ahead(&mut s, 7.0, true));
    {
        let o = s.game.world.object_mut(other).unwrap();
        o.position[0] += 4.0;
        o.name = "Other".to_string();
    }
    s.update(DT);
    hover_on(&mut s, foe);
    let beside = pixel_of(&mut s, other);

    // Across the other one on the way: too soon to count.
    s.hover(NOWHERE, SIZE);
    s.update(DT);
    s.hover(beside, SIZE);
    s.update(DT);
    assert!(target_name(&mut s).starts_with("Someone"), "in the grace");

    // Stopped on it past the grace, it takes them.
    for _ in 0..20 {
        s.update(DT);
    }
    let beside = pixel_of(&mut s, other);
    s.hover(beside, SIZE);
    s.update(DT);
    assert!(target_name(&mut s).starts_with("Other"), "past the grace");
}

#[test]
fn the_slots_go_with_a_click_away_a_death_or_the_target_out_of_view() {
    let Some(mut s) = stage("slots_go", "") else {
        return;
    };
    let pc = s.game.world.pc;
    let foe = ahead(&mut s, 7.0, true);
    let kept = |s: &mut Stage| {
        s.update(DT);
        hover_on(s, foe);
        s.hover(NOWHERE, SIZE);
        s.update(DT);
        assert!(target_name(s).starts_with("Someone"));
    };

    // A click away.
    kept(&mut s);
    s.click(NOWHERE, SIZE);
    s.update(DT);
    assert!(!target_name(&mut s).starts_with("Someone"), "clicked away");
    s.game.world.stop_attacking(pc);

    // Out of view, behind the camera.
    kept(&mut s);
    let home = s.game.world.object(foe).unwrap().position;
    s.game.world.object_mut(foe).unwrap().position = [
        SPOT[0] - FACING.cos() * 12.0,
        SPOT[1] - FACING.sin() * 12.0,
        SPOT[2],
    ];
    s.update(DT);
    assert!(!target_name(&mut s).starts_with("Someone"), "out of view");
    s.game.world.object_mut(foe).unwrap().position = home;

    // Dead.
    kept(&mut s);
    let hp = s.game.world.object(foe).unwrap().hp;
    s.game.world.hurt(pc, foe, hp + 1);
    s.update(DT);
    assert!(!target_name(&mut s).starts_with("Someone"), "dead");
}

/// How solid the most solid pixel of the overlay is, in the middle of a plate.
fn solid_at(s: &mut Stage, at: (i32, i32)) -> u8 {
    let o = s.overlay().expect("the HUD");
    let k = o.height as f32 / 480.0;
    let (cx, top) = ((at.0 as f32 * k) as i32, (at.1 as f32 * k) as i32);
    let mut most = 0;
    for y in top..top + (12.0 * k) as i32 {
        for x in cx - (20.0 * k) as i32..cx + (20.0 * k) as i32 {
            if x >= 0 && y >= 0 && (x as u32) < o.width && (y as u32) < o.height {
                most = most.max(o.rgba[((y as u32 * o.width + x as u32) * 4 + 3) as usize]);
            }
        }
    }
    most
}

#[test]
fn the_label_of_remains_with_nothing_left_shows_at_a_third_of_its_strength() {
    let Some(mut s) = stage("empty", "") else {
        return;
    };
    let pc = s.game.world.pc;
    let foe = ahead(&mut s, 2.0, true);
    let w = &mut s.game.world;
    let sword = w.spawn_item("g_w_lngswrd01");
    w.object_mut(foe).unwrap().inventory.push(sword);
    let hp = w.object(foe).unwrap().hp;
    w.hurt(pc, foe, hp + 1);
    s.update(DT);
    let label = |s: &Stage| {
        s.plates_drawn()
            .iter()
            .find(|p| p.text.starts_with("Remains"))
            .cloned()
            .expect("a label")
    };
    let full = label(&s);
    assert!(
        full.opacity == 1.0 && !full.text.contains("[empty]"),
        "{full:?}"
    );
    let solid = solid_at(&mut s, (full.x, full.y));
    assert!(solid > 200, "something on it reads whole: {solid}");

    // Emptied in play, it fades as it goes.
    let w = &mut s.game.world;
    w.opened = Some(foe);
    for (item, _) in w.container_items() {
        w.take_item(item);
    }
    w.close_container();
    s.update(DT);
    let faded = label(&s);
    assert!(faded.text.contains("[empty]"), "{faded:?}");
    assert_eq!(faded.opacity, 0.33);
    let thin = solid_at(&mut s, (faded.x, faded.y));
    assert!(
        (60..=100).contains(&thin),
        "33% of what is solid, 255: {thin}"
    );
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
