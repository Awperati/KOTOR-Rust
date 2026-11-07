//! How often a new character and Trask beat Darth Bandon, at levels 1 and 2 (the owner's added branch: see `story_branches.rs`).
//! Usage: bandon_odds [fights] [health,...] [trask damage,...] [full hit points,...] [share of his blows,...]. The player only attacks: no powers, grenades or medpacks.
use kotor_archive::Install;
use kotor_game::{Action, Game, World};

/// What came of a fight: the player won, and each side's blows and what they did.
#[derive(Default)]
struct Result {
    won: bool,
    seconds: f32,
    trask_blows: usize,
    trask_hit: i32,
    player_hit: i32,
}

fn fight(
    class: i32,
    xp: i32,
    seed: u64,
    health: f32,
    damage: i32,
    full: i32,
    blow: f32,
) -> Option<Result> {
    let dir = kotor_core::install_dir()?;
    let mut inst = Install::open(&dir).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.interactive_dialogue = true;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", class, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    w.extended_story = true;
    w.story.bandon_health = Some(health);
    w.story.trask_damage = Some(damage);
    w.story.bandon_full = Some(full);
    w.story.bandon_blows = Some(blow);
    let pc = w.pc;
    if xp > 0 {
        w.grant_xp(pc, xp);
    }
    let mut g = Game::new(w);
    for _ in 0..600 {
        g.step_once();
        if g.world.in_dialogue() {
            g.world.dialogue_choose(0);
        }
    }
    // Where a player and Trask stand when the door opens on him.
    let trask = g.world.by_tag("end_trask")[0];
    let bandon = g.world.by_tag("end_bandon")[0];
    g.world.teleport(trask, [42.1, 107.2, 0.0]);
    g.world.teleport(bandon, [42.2, 103.8, 0.0]);
    g.world.teleport(pc, [42.6, 109.0, 0.0]);
    let scene = g.world.by_tag("end01_sceneobj03")[0];
    g.world.queue(
        scene,
        Action::StartConversation {
            target: pc,
            dialogue: "end_cut01".to_string(),
            anywhere: true,
        },
    );
    let mut answered = false;
    for _ in 0..20000 {
        g.step_once();
        let w = &mut g.world;
        if w.conversation_paused() {
            continue;
        }
        if let Some((_, _, _, replies)) = w.dialogue_view() {
            let pick = replies
                .iter()
                .position(|r| r.contains("together"))
                .unwrap_or(0);
            answered |= pick > 0;
            w.dialogue_choose(pick);
        } else if answered && !w.in_dialogue() {
            break;
        }
    }
    g.world.seed_dice(seed);
    let began = g.world.time;
    let from = g.world.combat.log.len();
    let mut r = Result::default();
    for _ in 0..(600 * 60) {
        g.step_once();
        let w = &mut g.world;
        // What Trask has left to say from the opening, which this measure skips.
        if w.dialogue_view().is_some() && !w.conversation_paused() {
            w.dialogue_choose(0);
        }
        let over = w.game_over || w.object(pc).is_some_and(|o| o.dead || o.down);
        let fled = w.by_tag("end_bandon").is_empty() || w.object(bandon).is_some_and(|o| o.hp <= 1);
        if over || fled {
            r.won = fled && !over;
            break;
        }
        if !w.combat.target.contains_key(&pc) {
            w.attack(pc, bandon);
        }
    }
    r.seconds = g.world.time - began;
    for l in &g.world.combat.log[from..] {
        if l.attacker == trask {
            r.trask_blows += 1;
            r.trask_hit += l.damage;
        } else if l.attacker == pc {
            r.player_hit += l.damage;
        }
    }
    Some(r)
}

fn list<T: std::str::FromStr + Clone>(arg: Option<String>, default: &[T]) -> Vec<T> {
    match arg {
        Some(a) => a.split(',').filter_map(|x| x.parse().ok()).collect(),
        None => default.to_vec(),
    }
}

/// A level 1 fight against one Sith trooper of this module's, beside Trask, for comparison.
fn trooper(class: i32, seed: u64) -> Option<Result> {
    let dir = kotor_core::install_dir()?;
    let mut inst = Install::open(&dir).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.interactive_dialogue = true;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", class, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    let pc = w.pc;
    let mut g = Game::new(w);
    for _ in 0..600 {
        g.step_once();
        if g.world.in_dialogue() {
            g.world.dialogue_choose(0);
        }
    }
    let w = &mut g.world;
    let trask = w.by_tag("end_trask")[0];
    let foe = w
        .objects
        .values()
        .find(|o| {
            o.tag.starts_with("end_sith")
                && o.kind == kotor_game::Kind::Creature
                && !o.dead
                && o.hp > 1
        })
        .map(|o| o.id)?;
    w.teleport(trask, [42.1, 107.2, 0.0]);
    w.teleport(foe, [42.2, 103.8, 0.0]);
    w.teleport(pc, [42.6, 109.0, 0.0]);
    if let Some(o) = w.object_mut(pc) {
        o.min_one_hp = false;
    }
    if let Some(o) = w.object_mut(foe) {
        o.min_one_hp = false;
        o.faction = 1;
    }
    w.seed_dice(seed);
    let began = w.time;
    let from = w.combat.log.len();
    let mut r = Result::default();
    for _ in 0..(300 * 60) {
        g.step_once();
        let w = &mut g.world;
        if w.dialogue_view().is_some() && !w.conversation_paused() {
            w.dialogue_choose(0);
        }
        let over = w.game_over || w.object(pc).is_some_and(|o| o.dead || o.down);
        let gone = w.object(foe).is_none_or(|o| o.dead || o.hp <= 0);
        if over || gone {
            r.won = gone && !over;
            break;
        }
        if !w.combat.target.contains_key(&pc) {
            w.attack(pc, foe);
        }
        if !w.combat.target.contains_key(&foe) {
            w.attack(foe, pc);
        }
        if !w.combat.target.contains_key(&trask) {
            w.attack(trask, foe);
        }
    }
    r.seconds = g.world.time - began;
    for l in &g.world.combat.log[from..] {
        if l.attacker == trask {
            r.trask_blows += 1;
            r.trask_hit += l.damage;
        } else if l.attacker == pc {
            r.player_hit += l.damage;
        }
    }
    Some(r)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let n: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(30);
    let healths: Vec<f32> = list(args.next(), &[kotor_game::BANDON_HEALTH]);
    let damages: Vec<i32> = list(args.next(), &[kotor_game::TRASK_DAMAGE]);
    let fulls: Vec<i32> = list(args.next(), &[kotor_game::BANDON_FULL_HEALTH]);
    let blows: Vec<f32> = list(args.next(), &[kotor_game::BANDON_BLOWS]);
    println!("--- one Sith trooper, beside Trask, level 1");
    for (name, class) in [("soldier", 0), ("scout", 1), ("scoundrel", 2)] {
        let (mut wins, mut time) = (0, 0.0);
        for k in 0..n {
            if let Some(r) = trooper(class, k + 1) {
                wins += r.won as u64;
                time += r.seconds;
            }
        }
        println!(
            "{name}: won {wins} of {n}, {:.0} s a fight",
            time / n as f32
        );
    }
    for &full in &fulls {
        for &blow in &blows {
            for &health in &healths {
                for &damage in &damages {
                    println!("--- Bandon {full} hit points whole, starts at {health}, his blows {blow}, Trask adds {damage}");
                    for (name, class) in [("soldier", 0), ("scout", 1), ("scoundrel", 2)] {
                        let (mut wins, mut time) = (0, 0.0);
                        for k in 0..n {
                            if let Some(r) = fight(class, 0, k + 1, health, damage, full, blow) {
                                wins += r.won as u64;
                                time += r.seconds;
                            }
                        }
                        println!(
                            "{name} level 1: won {wins} of {n}, {:.0} s a fight",
                            time / n as f32
                        );
                    }
                }
            }
        }
    }
}
