//! A fight with a soldier who resists blaster fire, rendered offscreen (no window, no screen capture). The player's
//! blaster hits a plain soldier, then a second one wearing 75% blaster immunity; damage per hit is printed for both,
//! and a frame is taken in the middle of the second fight.
//!
//!   cargo run --release --example resist_shots -- [out_dir] [width] [height]
use kotor_game::{EffectRec, DURATION_PERMANENT};
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;
const BLASTER: f32 = 4096.0;

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
    for _ in 0..3000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
    let (pc, at, heading) = {
        let w = &s.game.world;
        (w.pc, w.object(w.pc).unwrap().position, w.pc_mover.heading)
    };
    s.yaw = heading + std::f32::consts::FRAC_PI_2;
    // The soldier's defence is 23 against a first-level aim of +3, so the player is given +20 to let shots land.
    {
        let w = &mut s.game.world;
        let mut e = EffectRec::new("EffectAttackIncrease");
        e.nums = vec![20.0];
        w.effects.push(e);
        let h = w.effects.len() as u32;
        w.apply_effect(pc, pc, h, DURATION_PERMANENT, 0.0);
    }
    for resists in [false, true] {
        let w = &mut s.game.world;
        let gun = w.spawn_item("g_w_blstrrfl001");
        let _ = w.equip(pc, gun, Some(4));
        let spot = [
            at[0] + heading.cos() * 4.0,
            at[1] + heading.sin() * 4.0,
            at[2],
        ];
        let foe = w.spawn_creature("n_calonord", spot);
        let o = w.object_mut(foe).unwrap();
        (o.hp, o.max_hp) = (900, 900);
        if resists {
            let mut e = EffectRec::new("EffectDamageImmunityIncrease");
            e.nums = vec![BLASTER, 75.0];
            w.effects.push(e);
            let h = w.effects.len() as u32;
            w.apply_effect(foe, foe, h, DURATION_PERMANENT, 0.0);
        }
        let first = w.combat.log.len();
        w.attack(pc, foe);
        let mut framed = false;
        for i in 0..(40.0 / DT) as usize {
            s.update(DT);
            if s.game.world.in_dialogue() {
                s.number_key(1);
                s.continue_line();
            }
            let hits = s.game.world.combat.log[first..]
                .iter()
                .filter(|l| l.attacker == pc && l.damage > 0)
                .count();
            if resists && !framed && hits >= 2 && !s.game.world.in_dialogue() {
                framed = true;
                let sc = s.dynamic_scene();
                let cam = s.camera();
                gpu.set_particles(s.particles());
                let mut img = gpu.render(&sc, &mut |n| s.texture(n), &cam).unwrap();
                if let Some(o) = s.overlay() {
                    composite(&mut img, o);
                }
                let p = out.join(format!("resisting_foe_{height}p.png"));
                std::fs::write(&p, kotor_image::to_png(&img))?;
                println!("wrote {} at step {i}", p.display());
            }
            if hits >= 12 {
                break;
            }
        }
        let w = &s.game.world;
        println!(
            "log {} target {:?} pc {:?} foe {:?} hp {:?}",
            w.combat.log.len() - first,
            w.combat.target.get(&pc),
            w.object(pc).map(|o| o.position),
            w.object(foe).map(|o| o.position),
            w.object(foe).map(|o| o.hp)
        );
        for l in &w.combat.log[first..] {
            println!(
                "  {:?} roll {} bonus {} def {} dmg {}",
                l.outcome, l.roll, l.bonus, l.defense, l.damage
            );
        }
        let dmg: Vec<i32> = s.game.world.combat.log[first..]
            .iter()
            .filter(|l| l.attacker == pc && l.damage > 0)
            .map(|l| l.damage)
            .collect();
        println!(
            "{} soldier: {} hits, damage per hit {dmg:?}, mean {:.1}",
            if resists { "resisting" } else { "plain" },
            dmg.len(),
            dmg.iter().sum::<i32>() as f32 / dmg.len().max(1) as f32
        );
        s.game.world.stop_attacking(pc);
        s.game.world.object_mut(foe).unwrap().hp = 0;
    }
    Ok(())
}
