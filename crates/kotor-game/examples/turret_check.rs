//! What a turret does to the player: its stats from its file against the player's armour, round by round.
//!
//!   cargo run --release -p kotor-game --example turret_check -- [template] [module]
use kotor_archive::Install;
use kotor_game::World;

fn main() {
    let template = std::env::args()
        .nth(1)
        .unwrap_or("tar10_thermturret".into());
    let module = std::env::args().nth(2).unwrap_or("tar_m10aa".into());
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = true;
    w.load_module(&module).unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    let id = w
        .objects
        .values()
        .find(|o| o.template.contains(&template) || o.tag.to_lowercase().contains(&template))
        .map(|o| o.id)
        .unwrap_or_else(|| {
            let names: Vec<String> = w
                .objects
                .values()
                .filter(|o| {
                    o.template.contains("turret") || o.tag.to_lowercase().contains("turret")
                })
                .map(|o| format!("{}/{}/{:?}", o.template, o.tag, o.kind))
                .collect();
            panic!("no such turret; turrets here: {names:?}")
        });
    let pc = w.pc;
    let t = w.object(id).unwrap().clone();
    println!(
        "{}: kind {:?} level {} hp {} faction {} perception {} natural_ac {} abilities {:?} equipped {:?} plot {}",
        t.template, t.kind, t.level, t.max_hp, t.faction, t.perception, t.natural_ac, t.abilities, t.equipped, t.plot
    );
    let wp = w.weapon_of(id);
    println!(
        "weapon: {wp:?} attack bonus {} base attack {} defense of the player {} (level {})",
        w.attack_bonus_of(id, 4),
        w.base_attack_of(id),
        w.defense_of(pc),
        w.object(pc).unwrap().level
    );
    println!("reputation turret->player {}", w.reputation(id, pc));
    let all: Vec<String> = w
        .objects
        .values()
        .filter(|o| o.template.starts_with("tar10_thermturr"))
        .map(|o| {
            format!(
                "#{} at {:.0},{:.0} faction {} hp {}",
                o.id, o.position[0], o.position[1], o.faction, o.hp
            )
        })
        .collect();
    println!("{} such turrets: {all:?}", all.len());
    let l = w.object(id).unwrap();
    println!(
        "its item {:?}: {:?}",
        l.equipped.get(&4),
        l.equipped
            .get(&4)
            .and_then(|i| w.object(*i))
            .map(|i| (i.template.clone(), i.base_item))
    );
    println!(
        "its second {:?}",
        l.equipped
            .get(&17)
            .and_then(|i| w.object(*i))
            .map(|i| (i.template.clone(), i.base_item))
    );
    // Fight it for 24 s: the player stands 8 m off with plenty of hit points, and we count what the turret does.
    let at = w.object(id).unwrap().position;
    w.teleport(pc, [at[0] - 8.0, at[1], at[2]]);
    let o = w.object_mut(pc).unwrap();
    (o.hp, o.max_hp) = (900, 900);
    let turrets: Vec<u32> = w
        .objects
        .values()
        .filter(|o| o.template.contains("turret") && !o.dead && o.faction == 1)
        .map(|o| o.id)
        .collect();
    println!("hostile turrets in the room: {}", turrets.len());
    let log0 = w.combat.log.len();
    w.attack(id, pc);
    let mut lost = Vec::new();
    for second in 0..24 {
        let before = w.object(pc).unwrap().hp;
        for _ in 0..20 {
            w.step(0.05);
        }
        lost.push(before - w.object(pc).unwrap().hp);
        let _ = second;
    }
    println!("hit points lost each second: {lost:?}");
    let by: Vec<_> = w.combat.log[log0..]
        .iter()
        .filter(|l| l.target == pc)
        .take(14)
        .map(|l| {
            format!(
                "r{} t{} roll{}+{} vs {} {:?} {}",
                l.round, l.attacker, l.roll, l.bonus, l.defense, l.outcome, l.damage
            )
        })
        .collect();
    println!(
        "{} attacks on the player, first: {by:#?}",
        w.combat.log[log0..]
            .iter()
            .filter(|l| l.target == pc)
            .count()
    );
}
