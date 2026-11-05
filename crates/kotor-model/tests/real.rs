use kotor_archive::Install;
use kotor_core::ResType;
use kotor_model::{parse_model, Node};

#[test]
fn every_base_model_parses() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let names = inst.list(ResType::MDL);
    assert!(names.len() > 1000, "{} models", names.len());
    let (mut ok, mut meshes, mut skins, mut verts, mut anims) = (0, 0, 0, 0usize, 0);
    let mut failures = Vec::new();
    let mut mismatched: Vec<String> = Vec::new();
    for n in &names {
        let mdl = inst.read(n, ResType::MDL).unwrap();
        let mdx = inst.read(n, ResType::MDX).unwrap_or_default();
        match parse_model(&mdl, &mdx) {
            Ok(m) => {
                ok += 1;
                anims += m.animations.len();
                let mut count = 0;
                m.root.walk(&mut |node: &Node| {
                    count += 1;
                    if let Some(mesh) = &node.mesh {
                        meshes += 1;
                        verts += mesh.positions.len();
                        if mesh.skin.is_some() {
                            skins += 1;
                        }
                        for p in &mesh.positions {
                            assert!(
                                p.iter().all(|c| c.is_finite() && c.abs() < 1.0e6),
                                "{n}: bad position {p:?}"
                            );
                        }
                    }
                });
                if m.supermodel.is_none() && count as u32 != m.node_count {
                    mismatched.push(format!("{n}: {count} vs header {}", m.node_count));
                }
            }
            Err(e) => failures.push(format!("{n}: {e}")),
        }
    }
    println!(
        "{ok}/{} models, {meshes} meshes, {skins} skins, {verts} vertices, {anims} animations",
        names.len()
    );
    println!(
        "failures ({}): {:#?}",
        failures.len(),
        &failures[..failures.len().min(15)]
    );
    println!(
        "node count mismatches without supermodel ({}): {:#?}",
        mismatched.len(),
        &mismatched[..mismatched.len().min(10)]
    );
    assert!(failures.is_empty());
}

fn printable(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
}

#[test]
fn light_and_emitter_headers_look_sane_in_every_model() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let (mut lights, mut emitters) = (0, 0);
    let mut updates = std::collections::BTreeMap::<String, usize>::new();
    let mut renders = std::collections::BTreeMap::<String, usize>::new();
    let mut blends = std::collections::BTreeMap::<String, usize>::new();
    let (mut flare_lights, mut max_priority, mut min_priority) = (0, 0, 0);
    for n in inst.list(ResType::MDL) {
        let mdl = inst.read(&n, ResType::MDL).unwrap();
        let mdx = inst.read(&n, ResType::MDX).unwrap_or_default();
        let m = parse_model(&mdl, &mdx).unwrap();
        m.root.walk(&mut |node: &Node| {
            if let Some(l) = &node.light {
                lights += 1;
                assert!(
                    l.flare_radius.is_finite() && l.flare_radius >= 0.0,
                    "{n}/{}: flare radius {}",
                    node.name,
                    l.flare_radius
                );
                assert!(
                    (-100..=100).contains(&l.priority),
                    "{n}/{}: priority {}",
                    node.name,
                    l.priority
                );
                max_priority = max_priority.max(l.priority);
                min_priority = min_priority.min(l.priority);
                assert!(
                    l.flare_textures.iter().all(|t| printable(t)),
                    "{n}: flare texture names"
                );
                if l.flare {
                    flare_lights += 1;
                    assert_eq!(
                        l.flare_textures.len(),
                        l.flare_sizes.len(),
                        "{n}/{}: flare array lengths",
                        node.name
                    );
                }
            }
            if let Some(e) = &node.emitter {
                emitters += 1;
                for s in [
                    &e.update,
                    &e.render,
                    &e.blend,
                    &e.texture,
                    &e.chunk_name,
                    &e.depth_texture,
                ] {
                    assert!(printable(s), "{n}/{}: unprintable string {s:?}", node.name);
                }
                assert!(
                    e.grid[0] < 1000 && e.grid[1] < 1000,
                    "{n}/{}: grid {:?}",
                    node.name,
                    e.grid
                );
                *updates.entry(e.update.clone()).or_default() += 1;
                *renders.entry(e.render.clone()).or_default() += 1;
                *blends.entry(e.blend.clone()).or_default() += 1;
            }
        });
    }
    println!("{lights} lights ({flare_lights} with flares, priority {min_priority}..={max_priority}), {emitters} emitters");
    println!("update {updates:?}\nrender {renders:?}\nblend {blends:?}");
    assert!(lights > 100 && emitters > 100);
    // The values are a small closed vocabulary; a wrong offset would produce junk here.
    assert!(
        updates.len() <= 8 && renders.len() <= 12 && blends.len() <= 6,
        "vocabulary too large"
    );
}

fn angle(a: [f32; 4], b: [f32; 4]) -> f32 {
    let d: f32 = (0..4).map(|i| a[i] * b[i]).sum::<f32>().abs().min(1.0);
    2.0 * d.acos().to_degrees()
}

#[test]
fn animation_keys_are_well_formed_and_sample_smoothly() {
    use kotor_model::unpack_quat;
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let (mut anims, mut keyed_nodes, mut packed_keys, mut packed_bad) = (0, 0, 0usize, 0usize);
    let mut max_n2: f32 = 0.0;
    let (mut unsorted, mut past_end) = (0, 0);
    let mut start_angles: Vec<f32> = Vec::new();
    let mut max_step: f32 = 0.0;
    let mut jumps = 0;
    for n in inst.list(ResType::MDL) {
        let mdl = inst.read(&n, ResType::MDL).unwrap();
        let mdx = inst.read(&n, ResType::MDX).unwrap_or_default();
        let m = parse_model(&mdl, &mdx).unwrap();
        for a in &m.animations {
            anims += 1;
            a.root.walk(&mut |node: &Node| {
                for c in &node.controllers {
                    if c.times.windows(2).any(|w| w[1] < w[0]) {
                        unsorted += 1;
                    }
                    if c.times.last().is_some_and(|&t| t > a.length + 1e-3) {
                        past_end += 1;
                    }
                    if c.kind == 20 && c.columns == 2 {
                        for v in &c.values {
                            let (_, n2) = unpack_quat(*v);
                            packed_keys += 1;
                            max_n2 = max_n2.max(n2);
                            if n2 > 1.02 {
                                packed_bad += 1;
                            }
                        }
                    }
                }
                keyed_nodes += 1;
            });
            // Pose at t = 0 against the model's own rest pose, for nodes that exist in both.
            let p0 = a.sample(&m, 0.0);
            for (name, np) in &p0 {
                if let Some(rest) = m.root.find(name) {
                    start_angles.push(angle(np.orientation, rest.orientation));
                }
            }
            // Successive samples never jump: step between neighbouring times stays small.
            let steps = 20;
            let mut prev = a.sample(&m, 0.0);
            for i in 1..=steps {
                let cur = a.sample(&m, a.length * i as f32 / steps as f32);
                for (name, np) in &cur {
                    if let Some(pp) = prev.get(name) {
                        let ang = angle(np.orientation, pp.orientation);
                        max_step = max_step.max(ang);
                        if ang > 120.0 {
                            jumps += 1;
                        }
                    }
                }
                prev = cur;
            }
        }
    }
    start_angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |p: f32| start_angles[((start_angles.len() - 1) as f32 * p) as usize];
    println!("{anims} animations, {keyed_nodes} animated nodes, unsorted {unsorted}, past end {past_end}");
    println!("{packed_keys} packed keys, max |xyz|^2 {max_n2:.4}, over 1.02: {packed_bad}");
    println!(
        "t=0 vs rest angle (deg): median {:.1}, p90 {:.1}, p99 {:.1} over {} nodes; max step {max_step:.1}, jumps>120deg {jumps}",
        pct(0.5), pct(0.9), pct(0.99), start_angles.len()
    );
    assert!(anims > 1000);
    assert_eq!(unsorted, 0, "key times must not go backwards");
    assert_eq!(
        packed_bad, 0,
        "packed quaternion layout is wrong if xyz length exceeds 1"
    );
}

#[test]
fn idle_pose_from_supermodel_keeps_a_body_in_shape() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    // Every player/party/NPC body model whose supermodel carries a `pause1` idle.
    let bodies: Vec<String> = inst
        .list(ResType::MDL)
        .into_iter()
        .filter(|n| n.starts_with("p_") || n.starts_with("n_"))
        .collect();
    let mut supers = std::collections::HashMap::new();
    let mut all: Vec<(f32, String, String)> = Vec::new();
    let (mut medians, mut maxes, mut worst) = (Vec::new(), Vec::new(), (0.0f32, String::new()));
    for body in &bodies {
        let mdl = inst.read(body, ResType::MDL).unwrap();
        let mdx = inst.read(body, ResType::MDX).unwrap_or_default();
        let m = parse_model(&mdl, &mdx).unwrap();
        let Some(sup_name) = m.supermodel.clone() else {
            continue;
        };
        if !supers.contains_key(&sup_name) {
            let sup = match inst.read(&sup_name, ResType::MDL) {
                Ok(d) => Some(
                    parse_model(&d, &inst.read(&sup_name, ResType::MDX).unwrap_or_default())
                        .unwrap(),
                ),
                Err(_) => None,
            };
            supers.insert(sup_name.clone(), sup);
        }
        let Some(Some(sup)) = supers.get(&sup_name) else {
            continue;
        };
        let Some(anim) = sup.animation("pause1") else {
            continue;
        };
        let rest = m.world_matrices(None);
        // Head models are a lone head node that the body places; only full skeletons are compared.
        if rest.len() < 40 {
            continue;
        }
        let posed = m.world_matrices(Some(&anim.sample(sup, 0.0)));
        let mut moved: Vec<f32> = rest
            .iter()
            .zip(&posed)
            .map(|(a, b)| {
                (0..3)
                    .map(|k| (a.1[k][3] - b.1[k][3]).powi(2))
                    .sum::<f32>()
                    .sqrt()
            })
            .collect();
        moved.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let max = moved[moved.len() - 1];
        medians.push(moved[moved.len() / 2]);
        maxes.push(max);
        all.push((max, body.clone(), sup_name.clone()));
        if max > worst.0 {
            worst = (max, body.clone());
        }
    }
    all.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    for (m, b, sname) in all.iter().take(8) {
        println!("  worst: {b} (super {sname}) max displacement {m:.2} m");
    }
    println!(
        "{} bodies posed with pause1 at t=0 (of {} p_/n_ models): worst max displacement {:.2} m ({})",
        medians.len(), bodies.len(), worst.0, worst.1
    );
    assert!(
        medians.len() >= 20,
        "only {} bodies had a usable idle",
        medians.len()
    );
    medians.sort_by(|a, b| a.partial_cmp(b).unwrap());
    maxes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (med_of_med, p90_max) = (medians[medians.len() / 2], maxes[maxes.len() * 9 / 10]);
    println!("median of medians {med_of_med:.3} m, p90 of max {p90_max:.3} m");
    assert!(
        med_of_med < 0.3 && p90_max < 1.0,
        "idle pose moves skeletons too far"
    );
}

#[test]
fn walk_and_run_cycles_keep_the_lowest_foot_on_the_floor() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let load = |inst: &mut Install, n: &str| {
        parse_model(
            &inst.read(n, ResType::MDL).unwrap(),
            &inst.read(n, ResType::MDX).unwrap_or_default(),
        )
        .unwrap()
    };
    let body = load(&mut inst, "p_bastilabb");
    let sup = load(&mut inst, body.supermodel.as_deref().unwrap());
    for name in ["walk", "run"] {
        let anim = sup
            .animation(name)
            .unwrap_or_else(|| panic!("no {name} animation"));
        let zs: Vec<f32> = (0..32)
            .map(|f| {
                let pose = anim.sample(&sup, anim.length * f as f32 / 32.0);
                let mats = body.world_matrices(Some(&pose));
                let z =
                    |n: &str| mats.iter().find(|m| m.0.eq_ignore_ascii_case(n)).unwrap().1[2][3];
                z("rfoot_g").min(z("lfoot_g"))
            })
            .collect();
        let mean = zs.iter().sum::<f32>() / zs.len() as f32;
        let sd = (zs.iter().map(|z| (z - mean).powi(2)).sum::<f32>() / zs.len() as f32).sqrt();
        println!(
            "{name}: lowest foot mean {mean:.3} m, sd {:.1} mm",
            sd * 1000.0
        );
        // A wrong key decoding makes the supporting foot bob or sink; a correct one keeps it level.
        if name == "walk" {
            assert!(sd < 0.03, "{name}: lowest foot varies by {sd} m");
            assert!(
                (0.0..0.3).contains(&mean),
                "{name}: lowest foot at {mean} m"
            );
        } else {
            // A run has a flight phase with both feet in the air, so only the mean is meaningful.
            assert!(mean < 0.5, "{name}: lowest foot at {mean} m");
        }
    }
}
