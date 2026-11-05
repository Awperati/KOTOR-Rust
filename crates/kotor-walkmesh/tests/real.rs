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

/// Small deterministic generator so failures reproduce.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (1u64 << 31) as f32
    }
}

#[test]
fn aabb_tree_queries_match_brute_force_on_every_area_walkmesh() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mut rng = Lcg(12345);
    let (mut meshes, mut leaves_total, mut floor_checks, mut near_checks, mut ray_checks) =
        (0, 0, 0, 0, 0);
    for name in inst.list(ResType::WOK) {
        let b = parse_bwm(&inst.read(&name, ResType::WOK).unwrap()).unwrap();
        let leaves = b.check_tree().unwrap_or_else(|e| panic!("{name}: {e}"));
        leaves_total += leaves;
        meshes += 1;
        let walk: Vec<usize> = (0..b.faces.len())
            .filter(|&i| b.faces[i].walkable())
            .collect();
        if walk.is_empty() {
            assert!(b.nearest_walkable([0.0; 3]).is_none(), "{name}");
            continue;
        }
        for k in 0..20 {
            let f = walk[(rng.next() * walk.len() as f32) as usize % walk.len()];
            let v = b.faces[f].vertices.map(|i| b.vertices[i as usize]);
            // A point inside the face, found with random barycentric weights.
            let (u, w) = (rng.next(), rng.next());
            let (u, w) = if u + w > 1.0 {
                (1.0 - u, 1.0 - w)
            } else {
                (u, w)
            };
            let x = v[0][0] + u * (v[1][0] - v[0][0]) + w * (v[2][0] - v[0][0]);
            let y = v[0][1] + u * (v[1][1] - v[0][1]) + w * (v[2][1] - v[0][1]);
            let slow = b.height_at(x, y);
            let fast = b.floor_at(x, y, f32::MAX).map(|(_, z)| z);
            match (slow, fast) {
                (Some(s), Some(t)) => assert!((s - t).abs() < 1e-4, "{name}: floor {s} vs {t}"),
                // Both miss on sliver triangles where float rounding pushes the sample just outside.
                (None, None) => continue,
                other => panic!("{name}: floor mismatch {other:?} at ({x}, {y})"),
            }
            floor_checks += 1;
            // A ray straight down onto the floor hits it at the right height.
            let z = fast.unwrap();
            let hit = b.raycast([x, y, z + 5.0], [0.0, 0.0, -1.0], 100.0, true);
            let hit = hit.unwrap_or_else(|| panic!("{name}: ray missed floor at ({x}, {y})"));
            assert!(
                (hit.point[2] - z).abs() < 1e-3,
                "{name}: ray z {} vs floor {z}",
                hit.point[2]
            );
            ray_checks += 1;
            // Nearest walkable to a point off the floor agrees with a full scan.
            if k < 5 {
                let p = [
                    x + (rng.next() - 0.5) * 6.0,
                    y + (rng.next() - 0.5) * 6.0,
                    z + (rng.next() - 0.5) * 4.0,
                ];
                let (_, q1) = b.nearest_walkable(p).unwrap();
                let (_, q2) = b.nearest_walkable_linear(p).unwrap();
                let d = |q: [f32; 3]| (0..3).map(|i| (q[i] - p[i]).powi(2)).sum::<f32>();
                assert!(
                    (d(q1) - d(q2)).abs() < 1e-3,
                    "{name}: nearest {} vs {}",
                    d(q1),
                    d(q2)
                );
                near_checks += 1;
            }
        }
    }
    println!("{meshes} meshes, {leaves_total} leaves, {floor_checks} floor, {ray_checks} ray, {near_checks} nearest checks");
    assert!(floor_checks > 5000);
}
