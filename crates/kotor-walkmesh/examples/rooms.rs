use kotor_core::ResType;
fn main() {
    let area = std::env::args().nth(1).unwrap();
    let (ex, ey): (f32, f32) = (
        std::env::args().nth(2).unwrap().parse().unwrap(),
        std::env::args().nth(3).unwrap().parse().unwrap(),
    );
    let mut inst = kotor_archive::Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let lyt = kotor_walkmesh::parse_lyt(&inst.read(&area, ResType::LYT).unwrap()).unwrap();
    for r in &lyt.rooms {
        let Ok(d) = inst.read(&r.model, ResType::WOK) else {
            println!("{} no wok", r.model);
            continue;
        };
        let b = kotor_walkmesh::parse_bwm(&d).unwrap();
        let mut mn = [f32::MAX; 3];
        let mut mx = [f32::MIN; 3];
        for v in &b.vertices {
            for k in 0..3 {
                mn[k] = mn[k].min(v[k]);
                mx[k] = mx[k].max(v[k]);
            }
        }
        println!(
            "{} pos {:?} bbox {:?}..{:?} h_local {:?} h_world {:?} bwm.position {:?}",
            r.model,
            r.position,
            mn,
            mx,
            b.height_at(ex - r.position[0], ey - r.position[1]),
            b.height_at(ex, ey),
            b.position
        );
    }
}
