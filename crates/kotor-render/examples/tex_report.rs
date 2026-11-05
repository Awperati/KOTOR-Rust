use std::collections::BTreeMap;
fn main() {
    let area = std::env::args().nth(1).unwrap();
    let mut inst = kotor_archive::Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.load_texture_packs().unwrap();
    let scene = kotor_render::load_area(&mut inst, &area).unwrap();
    let mut names: BTreeMap<String, (usize, bool)> = BTreeMap::new();
    for m in &scene.meshes {
        for t in [&m.texture, &m.lightmap].into_iter().flatten() {
            let e = names.entry(t.to_ascii_lowercase()).or_insert((0, false));
            e.0 += m.triangles.len();
        }
    }
    let none = scene.meshes.iter().filter(|m| m.texture.is_none()).count();
    println!("{} meshes without a diffuse texture name", none);
    for m in scene.meshes.iter().filter(|m| m.texture.is_none()) {
        println!(
            "  untextured: {} tris {} lightmap {:?} uvs {} first vertex {:?}",
            m.name,
            m.triangles.len(),
            m.lightmap,
            m.uvs.len(),
            m.positions.first()
        );
    }
    for (n, e) in names.iter_mut() {
        e.1 = kotor_render::load_texture(&mut inst, n).is_some();
    }
    for (n, (tris, ok)) in &names {
        if !ok {
            println!("MISSING {n} ({tris} tris)");
        }
    }
    println!(
        "{} distinct names, {} missing",
        names.len(),
        names.values().filter(|e| !e.1).count()
    );
}
// (appended) details of untextured meshes
