use kotor_archive::Install;
use kotor_core::ResType;
use kotor_walkmesh::{parse_bwm, parse_lyt, parse_vis};

#[test]
fn all_base_walkmeshes_parse_and_are_consistent() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mut n = 0;
    for ty in [ResType::WOK, ResType::DWK, ResType::PWK] {
        for name in inst.list(ty) {
            let b = parse_bwm(&inst.read(&name, ty).unwrap())
                .unwrap_or_else(|e| panic!("{name}.{ty}: {e}"));
            // In area walkmeshes walkable faces come first (door/placeable ones are unordered).
            if ty == ResType::WOK {
                let first_blocked = b
                    .faces
                    .iter()
                    .position(|f| !f.walkable())
                    .unwrap_or(b.faces.len());
                assert!(
                    b.faces[first_blocked..].iter().all(|f| !f.walkable()),
                    "{name}: walkable after blocked face"
                );
            }
            for f in &b.faces {
                let len = f.normal.iter().map(|c| c * c).sum::<f32>().sqrt();
                assert!(
                    (len - 1.0).abs() < 0.01 || len == 0.0,
                    "{name}: normal length {len}"
                );
            }
            n += 1;
        }
    }
    assert!(n > 1500, "{n} walkmeshes");
}

#[test]
fn module_layout_visibility_and_walkmesh_height() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let lyt = parse_lyt(&inst.read("m01aa", ResType::LYT).unwrap()).unwrap();
    assert!(lyt.rooms.len() >= 5, "{} rooms", lyt.rooms.len());
    let vis = parse_vis(&inst.read("m01aa", ResType::VIS).unwrap()).unwrap();
    assert_eq!(vis.len(), lyt.rooms.len());
    let mut checked = 0;
    // Every room's walkmesh is found and covers some floor at its own centre of mass.
    for room in &lyt.rooms {
        let data = inst
            .read(&room.model, ResType::WOK)
            .unwrap_or_else(|e| panic!("{}: {e}", room.model));
        let b = parse_bwm(&data).unwrap();
        assert_eq!(b.kind, 1, "{} is not an area walkmesh", room.model);
        let Some(f) = b.faces.iter().find(|f| f.walkable()) else {
            continue;
        };
        checked += 1;
        let v = f.vertices.map(|i| b.vertices[i as usize]);
        let (cx, cy) = (
            (v[0][0] + v[1][0] + v[2][0]) / 3.0,
            (v[0][1] + v[1][1] + v[2][1]) / 3.0,
        );
        let z = b.height_at(cx, cy).expect("face centre has no height");
        let zs = (v[0][2] + v[1][2] + v[2][2]) / 3.0;
        assert!((z - zs).abs() < 2.0, "{}: z {z} vs {zs}", room.model);
    }
    assert!(checked >= 3, "only {checked} rooms had walkable floor");
}
