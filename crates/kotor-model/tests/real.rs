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
