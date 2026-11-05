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
