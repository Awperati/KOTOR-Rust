//! Two modules joined at a door each have a pocket behind it, where the other stands. Drawn together the two floors lie in
//! one plane and fight for the pixels, which is the owner's jagged patches in Upper City South. So the neighbour is seen
//! through the doorway, and its picture is never added to this one's.
use kotor_archive::Install;
use kotor_game::seamless::prepare;
use kotor_game::World;
use kotor_gpu::stage::Stage;
use kotor_render::Scene;

const DT: f32 = 1.0 / 60.0;

fn triangles(s: &Scene) -> usize {
    s.meshes.iter().map(|m| m.triangles.len()).sum()
}

#[test]
fn a_door_with_a_pocket_shows_its_neighbour_through_the_doorway_only() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("tar_m02ac", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let before = triangles(s.static_scene());
    let site = s
        .game
        .world
        .sketch()
        .sites
        .into_iter()
        .find(|x| x.tag == "Apartments")
        .unwrap();
    let out = site.out.unwrap();
    let pc = s.game.world.pc;
    s.game.world.teleport(
        pc,
        [
            site.at[0] - out.cos() * 7.0,
            site.at[1] - out.sin() * 7.0,
            site.at[2],
        ],
    );
    let began = std::time::Instant::now();
    while s.game.world.seamless.attached.is_empty() && began.elapsed().as_secs() < 90 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let a = s.game.world.seamless.attached.values().next().unwrap();
    assert_eq!(a.module, "tar_m02aa", "{:?}", s.seam_notes());
    assert!(!a.portal, "it stands side by side for walking");
    assert!(a.seen_through_doorway());
    // The picture comes a piece a frame after the floor has joined.
    let began = std::time::Instant::now();
    while s.portal().is_none() && began.elapsed().as_secs() < 90 {
        s.update(DT);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(triangles(s.static_scene()), before, "nothing stitched on");
    let (seen, _) = s.portal().expect("the Apartments, through the doorway");
    assert!(triangles(seen) > 0);
}

#[test]
fn a_link_with_no_door_is_still_stitched_on() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let inst = Install::open(&dir).unwrap();
    let mut world = World::new(inst).unwrap();
    world.load_module("korr_m36aa").unwrap();
    let sketch = world.sketch();
    let site = sketch
        .sites
        .iter()
        .find(|x| x.tag.eq_ignore_ascii_case("k36_trg_k34"))
        .unwrap();
    let (a, _) = prepare(&dir, &sketch, site, &Default::default())
        .map_err(|e| e.to_string())
        .unwrap();
    assert!(!a.seen_through_doorway(), "open ground has no doorway");
}
