//! How often a new character and Trask beat Darth Bandon, at levels 1 and 2 (the owner's added branch: see `story_branches.rs`).
//! Usage: bandon_odds [fights] [health,...] [trask damage,...] [full hit points,...] [share of his blows,...] [player levels,...]. The player only attacks: no powers, grenades or medpacks.
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
    /// Blows of Bandon at the player: tried, and landed.
    bandon_tried: usize,
    bandon_landed: usize,
    player_max: i32,
    bandon_max: i32,
    player_defense: i32,
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
    // 0 hit points means his own rule: so many for each point of damage the player and Trask deal.
    w.story.bandon_full = (full > 0).then_some(full);
    w.story.bandon_per_damage = std::env::var("BANDON_K").ok().and_then(|k| k.parse().ok());
    w.story.bandon_blows = Some(blow);
    let pc = w.pc;
    if xp > 0 {
        w.grant_xp(pc, xp);
    }
    // The starting kit: the character begins with nothing worn, so each fights with one weapon of the Endar Spire's first
    // rooms (`BANDON_KIT`, a blaster pistol unless it says otherwise), and bare-handed if it says "none".
    {
        let kit = std::env::var("BANDON_KIT").unwrap_or_else(|_| "g_w_blstrpstl001".to_string());
        if kit != "none" {
            let item = w.spawn_item(&kit);
            w.carry(pc, item);
            w.equip(pc, item, Some(4)).ok()?;
        }
    }
    let mut g = Game::new(w);
    for _ in 0..600 {
        g.step_once();
        if g.world.in_dialogue() {
            g.world.dialogue_choose(0);
        }
    }
    // Where the pilot's walk-in has them when Trask asks.
    let trask = g.world.by_tag("end_trask")[0];
    let bandon = g.world.by_tag("end_bandon")[0];
    g.world.teleport(trask, [42.6, 111.9, 0.0]);
    g.world.teleport(bandon, [42.2, 100.6, 0.0]);
    g.world.teleport(pc, [43.6, 121.0, 0.0]);
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
    let mut last_hp = g.world.object(pc).map_or(0, |o| o.hp);
    for _ in 0..(600 * 60) {
        g.step_once();
        let w = &mut g.world;
        // What Trask has left to say from the opening, which this measure skips.
        if w.dialogue_view().is_some() && !w.conversation_paused() {
            w.dialogue_choose(0);
        }
        if std::env::var("BANDON_TRACE").is_ok() {
            let hp = w.object(pc).map_or(0, |o| o.hp);
            if hp != last_hp {
                eprintln!("t {:.1} player hp {last_hp} -> {hp}", w.time - began);
                last_hp = hp;
            }
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
    r.player_max = g.world.object(pc).map_or(0, |o| o.max_hp);
    r.player_defense = g.world.defense_of(pc);
    r.bandon_max = g.world.object(bandon).map_or(0, |o| o.max_hp);
    for l in &g.world.combat.log[from..] {
        if l.attacker == bandon {
            r.bandon_tried += 1;
            r.bandon_landed += (l.damage > 0) as usize;
        }
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
fn trooper(class: i32, xp: i32, seed: u64) -> Option<Result> {
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
    if xp > 0 {
        w.grant_xp(pc, xp);
    }
    // The starting kit: the character begins with nothing worn, so each fights with one weapon of the Endar Spire's first
    // rooms (`BANDON_KIT`, a blaster pistol unless it says otherwise), and bare-handed if it says "none".
    {
        let kit = std::env::var("BANDON_KIT").unwrap_or_else(|_| "g_w_blstrpstl001".to_string());
        if kit != "none" {
            let item = w.spawn_item(&kit);
            w.carry(pc, item);
            w.equip(pc, item, Some(4)).ok()?;
        }
    }
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
    // The player's level: 1 is a new character, and the others get the experience that level starts at.
    let levels: Vec<i32> = list(args.next(), &[1]);
    let xp_of = |level: i32| level * (level - 1) * 500;
    for &level in levels
        .iter()
        .filter(|_| std::env::var("NO_TROOPER").is_err())
    {
        println!("--- one Sith trooper, beside Trask, level {level}");
        for (name, class) in [("soldier", 0), ("scout", 1), ("scoundrel", 2)] {
            let (mut wins, mut time) = (0, 0.0);
            for k in 0..n {
                if let Some(r) = trooper(class, xp_of(level), k + 1) {
                    wins += r.won as u64;
                    time += r.seconds;
                }
            }
            println!(
                "{name}: won {wins} of {n}, {:.0} s a fight",
                time / n as f32
            );
        }
    }
    for &full in &fulls {
        for &blow in &blows {
            for &health in &healths {
                for &damage in &damages {
                    println!("--- Bandon {full} hit points whole, starts at {health}, his blows {blow}, Trask adds {damage}");
                    for &level in &levels {
                        for (name, class) in [("soldier", 0), ("scout", 1), ("scoundrel", 2)] {
                            let (mut wins, mut time) = (0, 0.0);
                            let (mut hit, mut taken, mut tried, mut landed) = (0, 0, 0, 0);
                            let (mut max_hp, mut defense, mut whole) = (0, 0, 0);
                            for k in 0..n {
                                if let Some(r) =
                                    fight(class, xp_of(level), k + 1, health, damage, full, blow)
                                {
                                    wins += r.won as u64;
                                    time += r.seconds;
                                    hit += r.player_hit;
                                    taken += r.trask_hit;
                                    tried += r.bandon_tried;
                                    landed += r.bandon_landed;
                                    max_hp = r.player_max;
                                    whole = r.bandon_max;
                                    defense = r.player_defense;
                                }
                            }
                            println!(
                                "{name} level {level}: won {wins} of {n}, {:.0} s a fight, the player dealt {} and Trask {} a fight; player {max_hp} hp, defense {defense}, Bandon {whole} hp, landed {} of {} blows",
                                time / n as f32,
                                hit / n as i32,
                                taken / n as i32,
                                landed / n as usize,
                                tried / n as usize
                            );
                        }
                    }
                }
            }
        }
    }
}
