//! The queue on the HUD and the Attack mode button, offscreen: four things queued while paused, the fight begun with the button
//! lit and a companion's portrait lit too, then the queue after two rounds.
//!
//!   cargo run --release --example queue_shots -- [out_dir] [width] [height]
use kotor_game::CombatAction;
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/combat", String::as_str));
    let width: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(720);
    std::fs::create_dir_all(&out)?;
    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let mut gpu = GpuRenderer::new(width, height)?;
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for _ in 0..3600 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    let (pc, home, heading) = {
        let w = &s.game.world;
        (w.pc, w.object(w.pc).unwrap().position, w.pc_mover.heading)
    };
    let w = &mut s.game.world;
    let others: Vec<_> = w
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Creature && o.id != pc)
        .map(|o| o.id)
        .collect();
    for id in others {
        w.object_mut(id).unwrap().dead = true;
        w.teleport(id, [0.0, 0.0, -300.0]);
    }
    w.combat.target.clear();
    {
        let p = w.object_mut(pc).unwrap();
        p.min_one_hp = true;
        p.feats.insert(28);
        p.spells.insert(23);
        (p.force, p.max_force) = (80, 80);
    }
    let faction = w.object(pc).unwrap().faction;
    let mate = w.spawn_creature(
        "n_calonord",
        [
            home[0] - heading.sin() * 1.0,
            home[1] + heading.cos() * 1.0,
            home[2],
        ],
    );
    {
        let m = w.object_mut(mate).unwrap();
        (m.faction, m.party_member, m.hp, m.max_hp) = (faction, true, 900, 900);
    }
    w.party.push(mate);
    let foe = w.spawn_creature(
        "n_calonord",
        [
            home[0] + heading.cos() * 4.0,
            home[1] + heading.sin() * 4.0,
            home[2],
        ],
    );
    {
        let f = w.object_mut(foe).unwrap();
        (f.hp, f.max_hp, f.faction, f.natural_ac) = (9000, 9000, 1, 0);
    }
    let grenade = w.spawn_item("g_w_fraggren01");
    w.carry(pc, grenade);
    s.yaw = heading + 2.6;
    s.distance = 7.0;
    s.pitch = 0.3;
    let shot = |s: &mut Stage, gpu: &mut GpuRenderer, name: &str| {
        s.update(DT);
        let sc = s.dynamic_scene();
        let cam = s.camera();
        gpu.set_particles(s.particles());
        let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let p = out.join(format!("queue_{name}_{height}p.png"));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        println!(
            "{name}: queue {:?} pictures {:?} leader lit {} mate lit {}",
            s.game.world.queue_of(s.game.world.pc),
            s.queue_pictures(),
            s.game.world.in_attack_mode(s.game.world.pc),
            s.game.world.in_attack_mode(mate)
        );
    };
    // The foe is far, so nothing fights yet. Paused, four things are queued.
    s.game.world.teleport(foe, [0.0, 0.0, -300.0]);
    s.toggle_pause();
    for a in [
        CombatAction::Attack,
        CombatAction::Feat(28),
        CombatAction::Power(23),
        CombatAction::Item(grenade),
    ] {
        s.game.world.queue_action(pc, a);
    }
    shot(&mut s, &mut gpu, "0_queued_paused");
    // Undo takes the last back.
    s.game.world.undo_queued(pc);
    shot(&mut s, &mut gpu, "1_after_undo");
    s.game.world.queue_action(pc, CombatAction::Item(grenade));
    // The fight starts. The leader's button and the companion's portrait light by themselves.
    s.game.world.teleport(
        foe,
        [
            home[0] + heading.cos() * 4.0,
            home[1] + heading.sin() * 4.0,
            home[2],
        ],
    );
    s.toggle_pause();
    s.game.world.attack(pc, foe);
    for _ in 0..20 {
        s.update(DT);
    }
    shot(&mut s, &mut gpu, "2_fight_begun");
    for _ in 0..150 {
        s.update(DT);
    }
    shot(&mut s, &mut gpu, "3_two_rounds_on");
    // A click on the lit button ends the fight.
    s.attack_mode_key();
    for _ in 0..20 {
        s.update(DT);
    }
    shot(&mut s, &mut gpu, "4_button_clicked_off");
    Ok(())
}
