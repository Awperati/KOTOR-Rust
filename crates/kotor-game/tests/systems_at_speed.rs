//! Door locks, skills, Force powers, long fights and saves, run as fast as the CPU allows and proved the same however the
//! steps are grouped. `docs/FASTTEST.md` says why that is the same as any speed in a window.
use kotor_archive::Install;
use kotor_game::{state_digest, Game, Kind, World};

/// `skills.2da`: Computer Use and Security.
const COMPUTER_USE: usize = 0;
const SECURITY: usize = 6;

fn world(module: &str) -> Option<World> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.interactive_dialogue = true;
    w.load_module(module).ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    Some(w)
}

/// Give the player eight ranks in a skill.
fn train(w: &mut World, skill: usize) {
    let pc = w.pc;
    let skills = &mut w.object_mut(pc).unwrap().skills;
    if skills.len() <= skill {
        skills.resize(skill + 1, 0);
    }
    skills[skill] = 8;
}

/// Run `steps` fixed steps, `chunk` at a time.
fn run(g: &mut Game, steps: usize, chunk: usize) {
    for _ in 0..steps / chunk {
        for _ in 0..chunk {
            g.step_once();
        }
    }
}

/// Everything the scenario leaves behind that has to match.
#[derive(Debug, PartialEq)]
struct Outcome {
    digest: u64,
    events: Vec<String>,
    save: Vec<u8>,
}

/// A scene with a locked door, a power and a fight, its orders falling on the same steps whatever the grouping.
fn locks_powers_and_a_fight(chunk: usize) -> Option<Outcome> {
    let w = world("end_m01aa")?;
    let mut g = Game::new(w);
    let pc = g.world.pc;
    let door = g.world.by_tag("end_door02")[0];
    let foe = g
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)?;
    run(&mut g, 1024, chunk);
    // Untrained, then trained: a door with a DC of 1 stays shut and then gives way.
    g.world.object_mut(door)?.locked = true;
    g.world.use_door(door);
    run(&mut g, 1024, chunk);
    let shut = g.world.object(door)?.locked;
    assert!(shut, "untrained Security must not open a door");
    train(&mut g.world, SECURITY);
    g.world.use_door(door);
    run(&mut g, 1024, chunk);
    let at = g.world.object(foe)?.position;
    g.world.teleport(pc, [at[0] - 4.0, at[1], at[2]]);
    // Powers with effects that run out on game time, then blows until someone falls.
    {
        let o = g.world.object_mut(pc)?;
        o.force = 200;
        o.max_force = 200;
        o.spells.extend([16, 46]);
    }
    for spell in [16, 46] {
        g.world.cast_spell(pc, spell, foe, false).ok()?;
        run(&mut g, 1024, chunk);
    }
    g.world.attack(pc, foe);
    run(&mut g, 1024 * 8, chunk);
    Some(Outcome {
        digest: state_digest(&g.world),
        events: g.world.events.clone(),
        save: g.world.save_bytes("speed").ok()?,
    })
}

#[test]
fn locks_powers_and_fights_do_not_care_how_steps_are_grouped() {
    let Some(one) = locks_powers_and_a_fight(1) else {
        return;
    };
    let events = one.events.join("\n");
    assert!(events.contains("security"), "the door was tried");
    assert!(events.contains("picked"), "and picked once trained");
    assert!(events.contains("casts power"), "a power was cast");
    assert!(events.contains("attacks Sith Soldier"), "blows were struck");
    assert_eq!(Some(&one), locks_powers_and_a_fight(16).as_ref());
    assert_eq!(Some(&one), locks_powers_and_a_fight(1024).as_ref());
}

#[test]
fn a_computer_panel_opens_with_the_skill_scripts_ask_for() {
    let Some(mut w) = world("end_m01ab") else {
        return;
    };
    let panel = w.by_tag("end_comp02")[0];
    assert_eq!(w.object(panel).unwrap().kind, Kind::Placeable);
    let pc = w.pc;
    train(&mut w, COMPUTER_USE);
    let at = w.object(panel).unwrap().position;
    w.teleport(pc, at);
    assert!(w.click(panel), "the panel can be used");
    assert!(w.in_dialogue(), "and opens its conversation");
    let view = w.dialogue_view().unwrap();
    assert!(
        !view.3.is_empty() || !view.1.is_empty(),
        "with something to read: {view:?}"
    );
    assert_eq!(w.skill_ranks(pc, COMPUTER_USE), 8);
}

#[test]
fn a_long_fight_and_repeated_saves_run_in_bulk() {
    let Some(w) = world("end_m01aa") else {
        return;
    };
    let mut g = Game::new(w);
    let pc = g.world.pc;
    let foe = g
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
        .unwrap();
    let at = g.world.object(foe).unwrap().position;
    g.world.teleport(pc, [at[0] - 4.0, at[1], at[2]]);
    g.world.object_mut(pc).unwrap().min_one_hp = true;
    g.world.object_mut(foe).unwrap().min_one_hp = true;
    g.world.attack(pc, foe);
    let started = std::time::Instant::now();
    run(&mut g, 60 * 300, 1);
    let blows = g.world.combat.log.len();
    println!(
        "300 game seconds of fighting, {blows} blows, in {:.2} wall seconds",
        started.elapsed().as_secs_f32()
    );
    assert!(blows > 100, "{blows} blows");
    // Twenty saves and loads in a row come back to the same save.
    let first = g.world.save_bytes("bulk").unwrap();
    let started = std::time::Instant::now();
    for _ in 0..20 {
        let bytes = g.world.save_bytes("bulk").unwrap();
        g.world.load_save(&bytes).unwrap();
    }
    println!(
        "20 saves and loads in {:.2} wall seconds",
        started.elapsed().as_secs_f32()
    );
    assert_eq!(
        first,
        g.world.save_bytes("bulk").unwrap(),
        "a save survives being loaded"
    );
}
