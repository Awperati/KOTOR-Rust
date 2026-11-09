//! Modern combat's blade through the window's loop: the chain, the arc, the lunge, the stagger, the guard and the time to kill.
use kotor_game::{Outcome, World};
use kotor_gpu::stage::Stage;

const SIZE: (u32, u32) = (640, 360);
const DT: f32 = 1.0 / 60.0;

fn stage(name: &str) -> Option<(Stage, std::path::PathBuf)> {
    kotor_core::install_dir()?;
    let mut s = Stage::open("danm14aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let file = std::env::temp_dir().join(format!("kotor_blows_{name}_{}.txt", std::process::id()));
    std::fs::write(&file, "").unwrap();
    s.use_settings_file(file.clone());
    s.set_view(SIZE);
    s.update(DT);
    for _ in 0..600 {
        if !s.game.world.in_dialogue() {
            break;
        }
        s.continue_line();
        s.number_key(1);
        s.update(DT);
    }
    let pc = s.game.world.pc;
    let me = s.game.world.object_mut(pc).unwrap();
    (me.hp, me.max_hp) = (90000, 90000);
    let sabre = s.game.world.spawn_item("g_w_lghtsbr01");
    // The character has not the feat for one: this is not about that.
    s.game
        .world
        .object_mut(pc)
        .unwrap()
        .equipped
        .insert(4, sabre);
    Some((s, file))
}

fn run(s: &mut Stage, frames: usize) {
    for _ in 0..frames {
        s.update(DT);
    }
}

/// An enemy `ahead` metres off, `off` radians from the middle of the view, that fights back with what it holds.
fn foe(s: &mut Stage, ahead: f32, off: f32, hp: i32) -> u32 {
    let at = s.game.world.pc_mover.pos;
    let way = s.yaw + off;
    let id = s.game.world.spawn_creature(
        "n_calonord",
        [at[0] + way.cos() * ahead, at[1] + way.sin() * ahead, at[2]],
    );
    let f = s.game.world.object_mut(id).unwrap();
    (f.hp, f.max_hp, f.faction) = (hp, hp, 1);
    id
}

/// A foe that only stands there, for counting.
fn dummy(s: &mut Stage, ahead: f32, off: f32, hp: i32) -> u32 {
    let at = s.game.world.pc_mover.pos;
    let pc = s.game.world.pc;
    let way = s.yaw + off;
    let w = &mut s.game.world;
    let id = w
        .objects
        .values()
        .find(|o| {
            o.kind == kotor_game::Kind::Creature
                && o.id != pc
                && !o.dead
                && !o.party_member
                && o.hp < 1000
        })
        .map(|o| o.id)
        .expect("a creature");
    w.teleport(
        id,
        [at[0] + way.cos() * ahead, at[1] + way.sin() * ahead, at[2]],
    );
    let f = w.object_mut(id).unwrap();
    (f.hp, f.max_hp, f.faction) = (hp, hp, 1);
    id
}

fn click(s: &mut Stage) {
    s.click((320.0, 20.0), SIZE);
}

fn blows_on(s: &Stage, id: u32, from: usize) -> usize {
    let w = &s.game.world;
    w.combat.log[from..]
        .iter()
        .filter(|l| l.attacker == w.pc && l.target == id)
        .count()
}

#[test]
fn clicks_in_time_chain_the_five_clips_and_a_wait_starts_over() {
    let Some((mut s, file)) = stage("chain") else {
        return;
    };
    s.set_combat_modern(true);
    let id = dummy(&mut s, 1.5, 0.0, 90000);
    let pc = s.game.world.pc;
    // The sword is drawn first: blows wait for the draw.
    s.game.world.set_attack_mode(true);
    run(&mut s, (kotor_game::DRAW_SECONDS / DT) as usize + 30);
    let logged = s.game.world.combat.log.len();
    let mut seen: Vec<String> = Vec::new();
    let began = s.game.world.time;
    for _ in 0..(4.5 / DT) as usize {
        click(&mut s);
        s.update(DT);
        let name = s
            .game
            .clip(pc)
            .map(|c| c.name.to_string())
            .unwrap_or_default();
        if name.starts_with("c2a") && seen.last() != Some(&name) {
            seen.push(name);
        }
    }
    let taken = s.game.world.time - began;
    assert_eq!(
        seen[..6],
        ["c2a1", "c2a2", "c2a3", "c2a4", "c2a5", "c2a1"],
        "{seen:?}"
    );
    // Each blow waits for the whole of the one before it: a clip as written is 1.467 s and plays at twice the speed, so 0.73 s.
    let blows = blows_on(&s, id, logged);
    let each = taken / blows as f32;
    assert!((0.7..0.9).contains(&each), "{blows} blows in {taken} s");
    // A wait starts it over.
    s.update(DT);
    run(&mut s, 120);
    assert_eq!(s.blow_step(), 0);
    click(&mut s);
    run(&mut s, 5);
    assert_eq!(s.game.clip(pc).unwrap().name, "c2a1");
    std::fs::remove_file(&file).ok();
}

#[test]
fn a_blow_strikes_the_arc_in_front_by_the_rolls_and_classic_does_none_of_it() {
    let Some((mut s, file)) = stage("arc") else {
        return;
    };
    let id = dummy(&mut s, 1.5, 0.0, 90000);
    s.attack_mode_key();
    run(&mut s, 2);
    // Classic: a click is no blow.
    let logged = s.game.world.combat.log.len();
    click(&mut s);
    run(&mut s, 60);
    assert_eq!(blows_on(&s, id, logged), 0, "Classic");
    let pc = s.game.world.pc;
    s.game.world.stop_attacking(pc);
    s.set_combat_modern(true);
    let at = s.game.world.pc_mover.pos;
    s.game.world.teleport(pc, at);
    run(&mut s, 2);
    // Two beside each other, and one behind.
    let behind = foe(&mut s, 1.6, std::f32::consts::PI, 90000);
    let beside = foe(&mut s, 1.6, 0.6, 90000);
    let logged = s.game.world.combat.log.len();
    for _ in 0..200 {
        click(&mut s);
        s.update(DT);
    }
    let w = &s.game.world;
    assert!(blows_on(&s, id, logged) >= 3);
    assert!(blows_on(&s, beside, logged) >= 3);
    assert_eq!(blows_on(&s, behind, logged), 0, "not behind");
    // Damage is a share of a blow, and at least 1.
    assert!(w.combat.log[logged..]
        .iter()
        .filter(|l| l.attacker == w.pc && l.outcome != Outcome::Miss)
        .all(|l| l.damage >= 1));
    std::fs::remove_file(&file).ok();
}

#[test]
fn a_blow_lunges_at_the_lock_and_stops_short_of_it() {
    let Some((mut s, file)) = stage("lunge") else {
        return;
    };
    s.set_combat_modern(true);
    let id = dummy(&mut s, 3.5, 0.0, 90000);
    run(&mut s, 2);
    s.lock_key();
    assert_eq!(s.locked_on(), Some(id));
    run(&mut s, 30);
    let gap = |s: &Stage| {
        let (m, o) = (
            s.game.world.pc_mover.pos,
            s.game.world.object(id).unwrap().position,
        );
        (m[0] - o[0]).hypot(m[1] - o[1])
    };
    let before = gap(&s);
    assert!(before > 3.0);
    click(&mut s);
    run(&mut s, 45);
    let after = gap(&s);
    assert!(after < before - 0.7, "{before} to {after}");
    // A chain closes in, and stops short of it.
    for _ in 0..240 {
        click(&mut s);
        s.update(DT);
    }
    let last = gap(&s);
    assert!((1.0..2.0).contains(&last), "{last}");
    std::fs::remove_file(&file).ok();
}

#[test]
fn what_a_chain_hits_staggers_only_a_moment_and_being_hit_does_not_stop_a_chain() {
    let Some((mut s, file)) = stage("stagger") else {
        return;
    };
    s.set_combat_modern(true);
    let id = foe(&mut s, 1.5, 0.0, 90000);
    let pc = s.game.world.pc;
    run(&mut s, 2);
    let (mut flinch, mut most) = (0.0f32, 0.0f32);
    let mut played_flinch = false;
    for _ in 0..600 {
        click(&mut s);
        s.update(DT);
        let on = s.game.clip(id).is_some_and(|c| c.name.ends_with("g1"));
        flinch = if on { flinch + DT } else { 0.0 };
        most = most.max(flinch);
        let me = s.game.clip(pc).map_or("", |c| c.name);
        played_flinch |= me.ends_with("g1");
    }
    assert!(most > 0.0, "it flinched");
    assert!(most < 0.9, "only a moment each: {most}");
    assert!(!played_flinch, "the chain goes on");
    std::fs::remove_file(&file).ok();
}

#[test]
fn right_held_with_a_blade_parries_what_fails_and_a_bolt_goes_back() {
    let Some((mut s, file)) = stage("guard") else {
        return;
    };
    s.set_combat_modern(true);
    let id = foe(&mut s, 6.0, 0.0, 90000);
    let pc = s.game.world.pc;
    run(&mut s, 2);
    s.set_aim_held(true);
    run(&mut s, 30);
    // With a blade the button is the Block, and not the aim.
    assert!(!s.aiming());
    assert!(s.game.world.pc_guard);
    // The player cannot be hit, here: every shot of theirs fails.
    let me = s.game.world.object_mut(pc).unwrap();
    me.level = 40;
    me.abilities[1] = 200;
    let logged = s.game.world.combat.log.len();
    let (mut parried, mut more_bolts) = (false, false);
    for _ in 0..1500 {
        let bolts = s.effects.bolts_fired;
        s.update(DT);
        // One frame: a shot and what comes back.
        more_bolts |= s.effects.bolts_fired >= bolts + 2;
        parried |= s
            .game
            .clip(pc)
            .is_some_and(|c| c.name.starts_with('c') && c.name.contains('p'));
        let o = s.game.world.object_mut(pc).unwrap();
        o.hp = 90000;
    }
    let w = &s.game.world;
    let fails = w.combat.log[logged..]
        .iter()
        .filter(|l| l.attacker == id && l.outcome == Outcome::Miss)
        .count();
    assert!(fails > 0, "it missed");
    assert!(parried, "a parry clip plays");
    assert!(more_bolts, "a bolt goes back");
    std::fs::remove_file(&file).ok();
}

/// The same fight in each mode: seconds to bring down the same foe, with the same sabre and the same character.
fn time_to_kill(modern: bool, hp: i32) -> Option<f32> {
    let (mut s, file) = stage(if modern { "ttk_modern" } else { "ttk_classic" })?;
    s.set_combat_modern(modern);
    let id = dummy(&mut s, 1.5, 0.0, hp);
    let pc = s.game.world.pc;
    run(&mut s, 2);
    if !modern {
        s.game.world.attack(pc, id);
    } else {
        s.attack_mode_key();
    }
    let began = s.game.world.time;
    let down = |w: &World| w.object(id).is_none_or(|o| o.dead || o.down);
    for _ in 0..(600.0 / DT) as usize {
        if modern {
            click(&mut s);
        }
        s.update(DT);
        if down(&s.game.world) {
            break;
        }
    }
    let taken = s.game.world.time - began;
    assert!(down(&s.game.world), "brought down in time");
    std::fs::remove_file(&file).ok();
    Some(taken)
}

#[test]
fn the_time_to_kill_is_near_classic_s() {
    let Some(classic) = time_to_kill(false, 150) else {
        return;
    };
    let modern = time_to_kill(true, 150).unwrap();
    eprintln!("time to kill: Classic {classic:.1} s, Modern {modern:.1} s");
    let ratio = modern / classic;
    // The owner sets the target. Until then a blade is somewhat faster than rounds, and not by much.
    assert!(
        (0.4..=1.1).contains(&ratio),
        "Modern {modern} against Classic {classic}"
    );
}
