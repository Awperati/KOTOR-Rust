use kotor_archive::Install;
use kotor_core::ResType;
use kotor_walkmesh::{parse_bwm, parse_lyt, Bwm, MoveParams, MoveStatus, Mover, NavMesh};

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

type Rooms = Vec<(String, Vec<[f32; 3]>)>;

/// The Endar Spire navmesh, its door positions, and one centroid list per room.
fn endar_spire() -> Option<(NavMesh, Vec<[f32; 3]>, Rooms)> {
    let dir = kotor_core::install_dir()?;
    let mut inst = Install::open(&dir).unwrap();
    inst.load_module("end_m01aa").unwrap();
    let lyt = parse_lyt(&inst.read("m01aa", ResType::LYT).unwrap()).unwrap();
    let mut meshes: Vec<Bwm> = Vec::new();
    let mut rooms: Rooms = Vec::new();
    for r in &lyt.rooms {
        let Ok(d) = inst.read(&r.model, ResType::WOK) else {
            continue;
        };
        let b = parse_bwm(&d).unwrap();
        let pts: Vec<[f32; 3]> = b
            .faces
            .iter()
            .filter(|f| f.walkable())
            .map(|f| {
                let v = f.vertices.map(|i| b.vertices[i as usize]);
                [0, 1, 2].map(|k| (v[0][k] + v[1][k] + v[2][k]) / 3.0)
            })
            .collect();
        rooms.push((r.model.clone(), pts));
        meshes.push(b);
    }
    let git = kotor_gff::Gff::parse(&inst.read("m01aa", ResType::GIT).unwrap()).unwrap();
    let doors: Vec<[f32; 3]> = git
        .root
        .list("Door List")
        .unwrap()
        .iter()
        .map(|d| {
            [
                d.float("X").unwrap() as f32,
                d.float("Y").unwrap() as f32,
                d.float("Z").unwrap() as f32,
            ]
        })
        .collect();
    Some((NavMesh::from_meshes(&meshes), doors, rooms))
}

fn len3(p: &[[f32; 3]]) -> f32 {
    p.windows(2)
        .map(|w| {
            (0..3)
                .map(|k| (w[0][k] - w[1][k]).powi(2))
                .sum::<f32>()
                .sqrt()
        })
        .sum()
}

const SPAWN: [f32; 3] = [15.4248, 20.1215, -1.275];

#[test]
fn navmesh_connects_the_endar_spire_rooms() {
    let Some((nav, _doors, rooms)) = endar_spire() else {
        return;
    };
    let mut reached = 0;
    let mut unreachable = Vec::new();
    for (name, pts) in &rooms {
        if pts.is_empty() {
            continue;
        }
        // Reachable if any of the room's first, middle or last face has a route from the spawn.
        let ok = [0, pts.len() / 2, pts.len() - 1]
            .iter()
            .any(|&i| nav.find_path(SPAWN, pts[i]).is_some());
        if ok {
            reached += 1;
        } else {
            unreachable.push(name.clone());
        }
    }
    println!(
        "{} faces; {reached} rooms reachable from the spawn; unreachable: {unreachable:?}",
        nav.face_count()
    );
    assert!(nav.face_count() > 400);
    assert!(
        reached >= 12,
        "only {reached} rooms reachable: {unreachable:?}"
    );
}

#[test]
fn routes_stay_on_the_floor_and_are_not_silly() {
    let Some((nav, _doors, rooms)) = endar_spire() else {
        return;
    };
    let mut rng = Lcg(777);
    let all: Vec<[f32; 3]> = rooms.iter().flat_map(|(_, p)| p.iter().copied()).collect();
    let (mut routes, mut samples, mut worst_ratio) = (0, 0, 1.0f32);
    for _ in 0..60 {
        let a = all[(rng.next() * all.len() as f32) as usize % all.len()];
        let b = all[(rng.next() * all.len() as f32) as usize % all.len()];
        let Some(path) = nav.find_path(a, b) else {
            continue;
        };
        routes += 1;
        let straight = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        let len = len3(&path);
        // String pulling never makes the route longer than walking from face centre to face centre.
        let faces = nav.face_path(a, b).unwrap();
        let mut chain = vec![a];
        chain.extend(faces.iter().map(|&f| nav.face_centroid(f)));
        chain.push(b);
        assert!(
            len <= len3(&chain) + 0.1,
            "pulled route {len} longer than the face chain {}",
            len3(&chain)
        );
        assert!(
            len >= straight - 0.05,
            "route shorter than the straight line: {len} < {straight}"
        );
        if straight > 3.0 {
            worst_ratio = worst_ratio.max(len / straight);
        }
        // Every point of the polyline, sampled every 10 cm, has floor under it (or within 2 cm of it).
        for w in path.windows(2) {
            let seg = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
            let n = (seg / 0.1).ceil().max(1.0) as usize;
            for i in 0..=n {
                let t = i as f32 / n as f32;
                // Heights are interpolated between corners, but ramps make the real floor differ, so only
                // require some floor within 3 m above the interpolated height.
                let (x, y, z) = (
                    w[0][0] + (w[1][0] - w[0][0]) * t,
                    w[0][1] + (w[1][1] - w[0][1]) * t,
                    w[0][2] + (w[1][2] - w[0][2]) * t,
                );
                let on = [
                    (0.0, 0.0),
                    (0.02, 0.0),
                    (-0.02, 0.0),
                    (0.0, 0.02),
                    (0.0, -0.02),
                ]
                .iter()
                .any(|(dx, dy)| nav.height_at(x + dx, y + dy, z + 3.0).is_some());
                assert!(
                    on,
                    "route leaves the floor at ({x:.2}, {y:.2}) on segment {w:?} of {path:?}"
                );
                samples += 1;
            }
        }
    }
    println!(
        "{routes} routes, {samples} samples on the floor, worst length ratio {worst_ratio:.2}"
    );
    assert!(routes >= 40, "only {routes} of 60 random pairs connected");
    // Some pairs are close in a straight line but far on foot (walls, two levels), so the ratio is
    // reported, not bounded.
}

#[test]
fn a_route_to_the_far_rooms_passes_through_doors() {
    let Some((nav, doors, rooms)) = endar_spire() else {
        return;
    };
    // Room m01aa_03a is at (46.6, 33.2): the route from the spawn has to go through doors on the way.
    let target = rooms.iter().find(|(n, _)| n == "m01aa_03a").unwrap().1[0];
    let path = nav.find_path(SPAWN, target).expect("route to m01aa_03a");
    let near_door = |d: &[f32; 3]| {
        path.windows(2).any(|w| {
            let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
            let l2 = (dx * dx + dy * dy).max(1e-9);
            let t = (((d[0] - w[0][0]) * dx + (d[1] - w[0][1]) * dy) / l2).clamp(0.0, 1.0);
            let (px, py) = (w[0][0] + t * dx, w[0][1] + t * dy);
            ((d[0] - px).powi(2) + (d[1] - py).powi(2)).sqrt() < 1.5
        })
    };
    let crossed = doors.iter().filter(|d| near_door(d)).count();
    println!(
        "route of {:.1} m with {} points passes {crossed} doors",
        len3(&path),
        path.len()
    );
    assert!(crossed >= 1, "no door on the way to another room");
    assert!(len3(&path) > 25.0);
}

#[test]
fn a_clicked_destination_is_walked_with_turning_and_stays_on_the_floor() {
    let Some((nav, _doors, rooms)) = endar_spire() else {
        return;
    };
    let target = rooms.iter().find(|(n, _)| n == "m01aa_03a").unwrap().1[0];
    // Facing +y while the goal is to the east: the mover must turn first.
    let mut m = Mover::new(SPAWN, std::f32::consts::FRAC_PI_2);
    assert!(m.go_to(&nav, target));
    let route_len = m.remaining();
    let p = MoveParams::default();
    let (mut t, mut last, mut walked, mut prev) = (0.0f32, MoveStatus::Idle, 0.0f32, m.pos);
    let (mut max_turn, mut prev_heading) = (0.0f32, m.heading);
    while last != MoveStatus::Arrived && t < 60.0 {
        last = m.step(&nav, &p, 0.05);
        let d = ((m.pos[0] - prev[0]).powi(2) + (m.pos[1] - prev[1]).powi(2)).sqrt();
        assert!(d <= p.speed * 0.05 + 1e-4, "stepped {d} m in 50 ms");
        walked += d;
        let mut dh = (m.heading - prev_heading).abs();
        if dh > std::f32::consts::PI {
            dh = std::f32::consts::TAU - dh;
        }
        max_turn = max_turn.max(dh);
        let on_floor = nav.height_at(m.pos[0], m.pos[1], m.pos[2] + 0.6).is_some()
            || nav
                .height_at(m.pos[0] + 0.02, m.pos[1], m.pos[2] + 0.6)
                .is_some()
            || nav
                .height_at(m.pos[0] - 0.02, m.pos[1], m.pos[2] + 0.6)
                .is_some();
        assert!(on_floor, "left the floor at {:?}", m.pos);
        prev = m.pos;
        prev_heading = m.heading;
        t += 0.05;
    }
    println!("walked {walked:.1} m of a {route_len:.1} m route in {t:.1}s, max turn per step {max_turn:.3} rad");
    assert_eq!(last, MoveStatus::Arrived);
    assert!(
        (walked - route_len).abs() < 0.5,
        "walked {walked} vs route {route_len}"
    );
    assert!(max_turn <= p.turn_rate * 0.05 + 1e-3);
    // Never faster than the speed limit allows; turning at corners adds a little time.
    assert!(
        t >= route_len / p.speed - 0.1 && t < route_len / p.speed + 10.0,
        "took {t}s"
    );
}
