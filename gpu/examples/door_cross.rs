//! Tries every closed door of each module named: walk, sprint, roll and jump at its middle and sides, from both faces. It says
//! which got past. No window: `cargo run --offline --release --manifest-path gpu\Cargo.toml --example door_cross -- tar_m02ac tar_m02aa`
use kotor_game::Kind;
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules: Vec<String> = std::env::args().skip(1).collect();
    for module in &modules {
        let mut s = match Stage::open(module, true) {
            Ok(s) => s,
            Err(e) => {
                println!("{module}: {e}");
                continue;
            }
        };
        s.enable_tutorials(false);
        s.enable_auto_pause(false);
        for _ in 0..1200 {
            s.update(DT);
            s.take_warm(64);
            if s.game.world.in_dialogue() {
                s.number_key(1);
                s.continue_line();
            } else if s.game.steps > 300 {
                break;
            }
        }
        let doors: Vec<_> = s
            .game
            .world
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door && !o.open)
            .map(|o| {
                (
                    o.id,
                    o.tag.clone(),
                    o.position,
                    o.door_axis,
                    o.door_half_width,
                )
            })
            .collect();
        let (mut tried, mut crossed) = (0, 0);
        for (id, tag, at, axis, half) in &doors {
            let out = [axis[1], -axis[0]];
            for facing in [1.0f32, -1.0] {
                for k in [-0.85f32, -0.4, 0.0, 0.4, 0.85] {
                    for how in ["sprint", "walk", "roll", "jump"] {
                        for modern in [true, false] {
                            if !modern && (how == "roll" || how == "jump") {
                                continue;
                            }
                            let start = [
                                at[0] - out[0] * facing * 2.5 + axis[0] * half * k,
                                at[1] - out[1] * facing * 2.5 + axis[1] * half * k,
                                at[2] + 1.0,
                            ];
                            let Some(z) = s
                                .game
                                .world
                                .nav
                                .as_ref()
                                .and_then(|n| n.walkable_at(start[0], start[1], start[2]))
                            else {
                                continue;
                            };
                            if (z - at[2]).abs() > 0.5 {
                                continue;
                            }
                            let way = [out[0] * facing, out[1] * facing];
                            let yaw = way[1].atan2(way[0]);
                            s.set_movement_modern(modern);
                            s.place_player([start[0], start[1], z], yaw);
                            s.steer = (0.0, 0.0, false);
                            s.update(DT);
                            s.yaw = yaw;
                            s.steer = (1.0, 0.0, how == "walk");
                            tried += 1;
                            if how == "roll" {
                                s.roll_key();
                            }
                            let frames = if how == "walk" { 200 } else { 70 };
                            let mut crossed_here = false;
                            let mut last = s.game.world.pc_mover.pos;
                            for f in 0..frames {
                                if how == "jump" && f % 45 == 5 {
                                    s.jump_key();
                                }
                                if how == "roll" && f % 45 == 40 {
                                    s.roll_key();
                                }
                                s.update(DT);
                                if s.game.world.in_dialogue() {
                                    s.continue_line();
                                    s.number_key(1);
                                }
                                let p = s.game.world.pc_mover.pos;
                                let side = (p[0] - at[0]) * way[0] + (p[1] - at[1]) * way[1];
                                let shut = s.game.world.object(*id).is_some_and(|o| !o.open);
                                let along =
                                    ((p[0] - at[0]) * axis[0] + (p[1] - at[1]) * axis[1]).abs();
                                if shut && side > 0.1 && along < *half && !crossed_here {
                                    crossed_here = true;
                                    let n = s.game.world.nav.as_ref().unwrap();
                                    println!(
                                        "  at frame {f}: p {p:?} side {side:.3} from {:?}: block whole way {} last {:?} speed {:.2} air {} attached {:?}",
                                        start,
                                        n.crosses_block([start[0], start[1], z], [p[0], p[1], p[2]]),
                                        last,
                                        s.game.world.pc_body.speed(),
                                        !s.game.world.pc_body.grounded,
                                        s.game.world.seamless.attached.values().map(|a| (a.module.clone(), a.portal, a.at, a.behind)).collect::<Vec<_>>(),
                                    );
                                }
                                last = p;
                                if s.game.world.module != *module {
                                    break;
                                }
                            }
                            if s.game.world.module != *module {
                                println!("{module} {tag} left the module by {how}");
                                break;
                            }
                            if crossed_here {
                                crossed += 1;
                                println!(
                                    "CROSSED {module} door {tag} half {half:.2} facing {facing} at {k} by {how} modern {modern} ended {:?}",
                                    s.game.world.pc_mover.pos
                                );
                            }
                        }
                    }
                }
            }
        }
        println!(
            "{module}: {} closed doors, {tried} tries, {crossed} crossed",
            doors.len()
        );
    }
    Ok(())
}
