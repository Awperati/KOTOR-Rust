use kotor_core::ResType;
fn load(inst: &mut kotor_archive::Install, n: &str) -> kotor_model::Model {
    kotor_model::parse_model(
        &inst.read(n, ResType::MDL).unwrap(),
        &inst.read(n, ResType::MDX).unwrap_or_default(),
    )
    .unwrap()
}
fn main() {
    let mut inst = kotor_archive::Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let m = load(&mut inst, "p_bastilabb");
    let sup = load(&mut inst, &m.supermodel.clone().unwrap());
    println!(
        "animations in super: {:?}",
        sup.animations
            .iter()
            .map(|a| a.name.clone())
            .take(12)
            .collect::<Vec<_>>()
    );
    let anim = sup.animation("pause1").unwrap();
    println!("pause1 len {} root {}", anim.length, anim.root_name);
    let pose = anim.sample(&sup, 0.0);
    let rest = m.world_matrices(None);
    let posed = m.world_matrices(Some(&pose));
    // Also the supermodel's own rest vs its posed.
    let srest = sup.world_matrices(None);
    let sposed = sup.world_matrices(Some(&pose));
    for i in 0..12.min(rest.len()) {
        let p = |v: &[(String, [[f32; 4]; 4])]| [v[i].1[0][3], v[i].1[1][3], v[i].1[2][3]];
        println!(
            "body {:>14}: rest {:?} posed {:?}",
            rest[i].0,
            p(&rest),
            p(&posed)
        );
    }
    for i in 0..6 {
        let p = |v: &[(String, [[f32; 4]; 4])]| [v[i].1[0][3], v[i].1[1][3], v[i].1[2][3]];
        println!(
            "super {:>13}: rest {:?} posed {:?}",
            srest[i].0,
            p(&srest),
            p(&sposed)
        );
    }
    let find = |v: &[(String, [[f32; 4]; 4])], n: &str| {
        v.iter()
            .find(|x| x.0.eq_ignore_ascii_case(n))
            .map(|x| [x.1[0][3], x.1[1][3], x.1[2][3]])
    };
    for n in ["head_g", "Head_g", "rhand", "lhand", "torso_g"] {
        println!(
            "{n}: body rest {:?} body posed {:?} super rest {:?} super posed {:?}",
            find(&rest, n),
            find(&posed, n),
            find(&srest, n),
            find(&sposed, n)
        );
    }
}
