//! Live frames for the combat builder's earlier fixes that were only tested headless: collision between the player
//! and a person (bug 15), a teammate in a fight (bug 21), remains that hold what the dead carried (bug 34), a party that
//! opens doors (bug 39) and Force Push (bug 36). Offscreen: no window, no screen capture.
//!
//!   cargo run --release --example sweep_shots -- [out_dir] [width] [height]
use kotor_game::FORCE_PUSH;
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const DT: f32 = 1.0 / 30.0;

struct Rig {
    s: Stage,
    gpu: GpuRenderer,
    out: std::path::PathBuf,
    tag: String,
}

impl Rig {
    fn shot(&mut self, name: &str) {
        let scene = self.s.dynamic_scene();
        let cam = self.s.camera();
        self.gpu.set_particles(self.s.particles());
        let mut img = self
            .gpu
            .render(&scene, &mut |n| self.s.texture(n), &cam)
            .unwrap();
        if let Some(o) = self.s.overlay() {
            composite(&mut img, o);
        }
        let p = self.out.join(format!("sweep_{name}_{}.png", self.tag));
        std::fs::write(&p, kotor_image::to_png(&img)).unwrap();
        println!("wrote {}", p.display());
    }

    fn run(&mut self, seconds: f32) {
        for _ in 0..(seconds / DT) as usize {
            self.s.update(DT);
            if self.s.game.world.in_dialogue() {
                self.s.number_key(1);
                self.s.continue_line();
            }
        }
    }

    fn pc_to(&mut self, at: [f32; 3], heading: f32) {
        let w = &mut self.s.game.world;
        let pc = w.pc;
        w.object_mut(pc).unwrap().position = at;
        w.pc_mover.pos = at;
        w.pc_mover.heading = heading;
        w.pc_mover.stop();
        self.s.yaw = heading;
    }
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

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
    let mut r = Rig {
        s,
        gpu,
        out,
        tag: format!("{height}p"),
    };
    r.run(50.0);
    let (pc, home, heading) = {
        let w = &r.s.game.world;
        (w.pc, w.object(w.pc).unwrap().position, w.pc_mover.heading)
    };
    println!("party {:?}", r.s.game.world.party);

    // Bug 36: Force Push.
    {
        r.s.game.world.close_container();
        r.pc_to(home, heading);
        let spot = [
            home[0] + heading.cos() * 1.6,
            home[1] + heading.sin() * 1.6,
            home[2],
        ];
        // From behind and above: the soldier flies away from the camera.
        r.s.yaw = heading;
        let w = &mut r.s.game.world;
        let foe = w.spawn_creature("n_calonord", spot);
        w.object_mut(foe).unwrap().hp = 900;
        w.object_mut(pc).unwrap().spells.insert(FORCE_PUSH);
        w.object_mut(pc).unwrap().force = 50;
        let kept = r.s.distance;
        r.s.distance = 9.0;
        r.s.pitch = 0.6;
        r.run(0.3);
        r.shot("36_push_before");
        let _ = r.s.game.world.cast_spell(pc, FORCE_PUSH, foe, true);
        r.run(0.15);
        r.shot("36_push_flying");
        r.run(0.3);
        r.shot("36_push_landing");
        r.run(1.5);
        r.shot("36_push_down");
        r.s.distance = kept;
        r.s.pitch = 0.0;
        let a = r.s.game.world.object(foe).unwrap().position;
        println!(
            "bug 36: foe moved {:.2} m, disabled {}",
            dist(a, spot),
            r.s.game.world.is_disabled(foe)
        );
    }

    // Bug 15: the player pushes on a person who stands in the way.
    {
        let spot = [
            home[0] + heading.cos() * 1.6,
            home[1] + heading.sin() * 1.6,
            home[2],
        ];
        let npc = r.s.game.world.spawn_creature("n_calonord", spot);
        r.s.game.world.object_mut(npc).unwrap().faction = 2;
        r.s.yaw = heading;
        r.s.steer = (1.0, 0.0, false);
        r.run(0.2);
        r.shot("15_collision_start");
        let mut least = f32::MAX;
        for i in 0..(3.0 / DT) as usize {
            r.s.update(DT);
            let w = &r.s.game.world;
            least = least.min(dist(
                w.object(pc).unwrap().position,
                w.object(npc).unwrap().position,
            ));
            if i == 20 {
                r.shot("15_collision_passing");
            }
        }
        r.shot("15_collision_end");
        let w = &r.s.game.world;
        let moved = dist(home, w.object(pc).unwrap().position);
        println!("bug 15: closest {least:.2} m (0.8 is touching), player moved {moved:.2} m");
        r.s.steer = (0.0, 0.0, false);
        r.s.game.world.object_mut(npc).unwrap().dead = true;
        r.s.game.world.object_mut(npc).unwrap().position = [0.0, 0.0, -100.0];
    }

    // Bug 39: a mate behind a shut door opens it and comes.
    {
        let w = &mut r.s.game.world;
        let id = w.by_tag("end_door01")[0];
        let d = w.object_mut(id).unwrap();
        d.scripts.clear();
        d.conversation.clear();
        d.open = false;
        d.locked = false;
        let o = w.object(id).unwrap().clone();
        let n = [-o.door_axis[1], o.door_axis[0]];
        let (p, z) = ([o.position[0], o.position[1]], o.position[2]);
        let at = |sign: f32, m: f32| [p[0] + n[0] * m * sign, p[1] + n[1] * m * sign];
        let nav = w.nav.as_ref().unwrap();
        let (a, b) = (at(1.0, 2.5), at(-1.0, 4.0));
        let near = nav.height_at(a[0], a[1], z + 0.4).map(|h| [a[0], a[1], h]);
        let far = nav.height_at(b[0], b[1], z + 0.4).map(|h| [b[0], b[1], h]);
        if let (Some(near), Some(far)) = (near, far) {
            let mate = w.spawn_creature("n_calonord", far);
            w.object_mut(mate).unwrap().party_member = true;
            w.party.push(mate);
            let face = (p[1] - near[1]).atan2(p[0] - near[0]);
            r.pc_to(near, face);
            r.s.yaw = face;
            r.run(0.3);
            r.shot("39_door_shut");
            for _ in 0..(14.0 / DT) as usize {
                r.s.game.world.object_mut(id).unwrap().locked = false;
                r.s.update(DT);
            }
            r.shot("39_door_open_mate_arrived");
            let w = &r.s.game.world;
            println!(
                "bug 39: door open {}, mate {:.1} m from the player",
                w.object(id).unwrap().open,
                dist(
                    w.object(mate).unwrap().position,
                    w.object(pc).unwrap().position
                )
            );
            r.s.game.world.party.retain(|p| *p != mate);
            r.s.game.world.object_mut(mate).unwrap().position = [0.0, 0.0, -100.0];
        } else {
            println!("bug 39: no floor each side of end_door01");
        }
    }

    // Bugs 21 and 34: a teammate in a fight attacks the foe; the foe's remains hold what it carried.
    {
        r.pc_to(home, heading);
        let w = &mut r.s.game.world;
        let mate = w.spawn_creature("n_calonord", [home[0] - 1.0, home[1] + 1.2, home[2]]);
        let m = w.object_mut(mate).unwrap();
        (m.party_member, m.faction) = (true, 2);
        w.party.push(mate);
        let foe = w.spawn_creature("n_calonord", [home[0] + 5.0, home[1], home[2]]);
        let f = w.object_mut(foe).unwrap();
        (f.hp, f.max_hp, f.faction) = (40, 40, 1);
        let carried = {
            let f = w.object(foe).unwrap();
            f.inventory.len() + f.equipped.len()
        };
        w.attack(foe, pc);
        r.s.yaw = heading + 0.6;
        let log0 = r.s.game.world.combat.log.len();
        let mut framed = false;
        for _ in 0..(40.0 / DT) as usize {
            r.s.update(DT);
            if r.s.game.world.in_dialogue() {
                r.s.number_key(1);
                r.s.continue_line();
            }
            let w = &r.s.game.world;
            let mate_hits = w.combat.log[log0..]
                .iter()
                .filter(|l| l.attacker == mate && l.target == foe)
                .count();
            if mate_hits >= 2 && !framed {
                framed = true;
                r.shot("21_teammate_attacks");
            }
            if r.s.game.world.object(foe).unwrap().dead {
                break;
            }
        }
        let w = &r.s.game.world;
        let count = |t: u32| {
            w.combat.log[log0..]
                .iter()
                .filter(|l| l.attacker == mate && l.target == t)
                .count()
        };
        println!(
            "bug 21: teammate attacks on the foe {}, on the player {}",
            count(foe),
            count(pc)
        );
        let left = w.object(foe).unwrap().inventory.len();
        println!(
            "bug 34: foe dead {}, carried {carried}, remains hold {left}, lootable {}",
            w.object(foe).unwrap().dead,
            w.lootable(foe)
        );
        if w.lootable(foe) {
            r.s.game.world.click(foe);
            r.run(4.0);
            r.shot("34_remains_opened");
        }
        r.s.game.world.party.retain(|p| *p != mate);
    }

    r.s.game.world.close_container();
    // Bug 25: a shooter and a target with only a short obstacle between them, which sight used to treat as a wall.
    {
        let w = &r.s.game.world;
        let nav = w.nav.as_ref().unwrap();
        let pos: Vec<[f32; 3]> = w
            .objects
            .values()
            .filter(|o| o.kind == kotor_game::Kind::Creature && !o.dead)
            .map(|o| o.position)
            .collect();
        let mut pair = None;
        'search: for i in 0..pos.len() {
            for j in i + 1..pos.len() {
                let (a, b) = (pos[i], pos[j]);
                let len = dist(a, b);
                if !(3.0..12.0).contains(&len) {
                    continue;
                }
                let steps = (len / 0.75).ceil().max(1.0) as usize;
                let top = a[2].max(b[2]) + 1.5;
                let old = !nav.crosses_block(a, b)
                    && (1..steps).all(|k| {
                        let t = k as f32 / steps as f32;
                        nav.walkable_at(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, top)
                            .is_some()
                    });
                if w.can_see_along(a, b) && !old {
                    pair = Some((a, b));
                    break 'search;
                }
            }
        }
        match pair {
            Some((a, b)) => {
                let face = (a[1] - b[1]).atan2(a[0] - b[0]);
                r.pc_to(b, face);
                r.s.yaw = face;
                let w = &mut r.s.game.world;
                let foe = w.spawn_creature("n_calonord", a);
                let f = w.object_mut(foe).unwrap();
                (f.hp, f.max_hp, f.faction) = (900, 900, 1);
                w.object_mut(pc).unwrap().min_one_hp = true;
                w.attack(foe, pc);
                let log0 = w.combat.log.len();
                let mut framed = false;
                for _ in 0..(8.0 / DT) as usize {
                    r.s.update(DT);
                    let n = r.s.game.world.combat.log[log0..]
                        .iter()
                        .filter(|l| l.attacker == foe && l.target == pc)
                        .count();
                    if n >= 1 && !framed {
                        framed = true;
                        r.shot("25_shot_over_obstacle");
                    }
                }
                let n = r.s.game.world.combat.log[log0..]
                    .iter()
                    .filter(|l| l.attacker == foe && l.target == pc)
                    .count();
                println!(
                    "bug 25: shooter {:.1} m away over a short obstacle fired {n} times at the player",
                    dist(a, b)
                );
            }
            None => println!("bug 25: no pair seen over an obstacle in this room"),
        }
    }
    Ok(())
}
