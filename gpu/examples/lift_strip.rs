//! Frames of a lift ride from the first update to the last: `lift_strip <module> <door tag> <out prefix>`. The module the lift
//! leads to is loaded ahead first, as it is when a player walks up to the door.
use kotor_gpu::stage::Stage;
use kotor_gpu::GpuRenderer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let module = a.get(1).cloned().unwrap_or_else(|| "tar_m02ab".into());
    let door_tag = a.get(2).cloned().unwrap_or_else(|| "tar02_lowdoor".into());
    let prefix = a.get(3).cloned().unwrap_or_else(|| "lift".into());
    let mut s = Stage::open(&module, true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.set_view((960, 540));
    let mut gpu = GpuRenderer::new(960, 540)?;
    s.update(1.0 / 60.0);
    assert!(s.stand_near(&door_tag), "no door {door_tag}");
    let door = s.game.world.by_tag(&door_tag)[0];
    let to = s.game.world.object(door).unwrap().link_module.clone();
    for _ in 0..3000 {
        s.update(1.0 / 60.0);
        if s.preloaded().is_some_and(|m| m.eq_ignore_ascii_case(&to)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    println!("loaded ahead: {:?}", s.preloaded());
    s.game.world.use_door(door);
    let mut portal_seen = 0;
    let mut static_seen = u64::MAX;
    let total = (s.game.world.lift_ride().map(|r| r.seconds).unwrap_or(4.0) * 60.0) as usize;
    let marks = [
        2,
        total / 5,
        2 * total / 5,
        3 * total / 5,
        4 * total / 5,
        total - 8,
    ];
    for frame in 0..total {
        s.update(1.0 / 60.0);
        if !marks.contains(&frame) {
            continue;
        }
        if static_seen != s.static_version {
            static_seen = s.static_version;
            let scene = s.static_scene().clone();
            gpu.set_static_scene(&scene, &mut |n| s.texture(n));
        }
        if portal_seen != s.portal_version() {
            portal_seen = s.portal_version();
            let other = s.portal().map(|(sc, ap)| (sc.clone(), ap));
            gpu.set_portal(other.as_ref().map(|(sc, ap)| (sc, *ap)), &mut |n| {
                s.texture(n)
            });
        }
        gpu.set_lift_view(s.lift_view());
        let scene = s.dynamic_scene();
        let cam = s.camera();
        let img = gpu.render(&scene, &mut |n| s.texture(n), &cam)?;
        let path = format!("{prefix}_{frame:03}.png");
        std::fs::write(&path, kotor_image::to_png(&img))?;
        println!(
            "{path} view {:?} dark {:.2}",
            s.lift_view(),
            s.lift_darkness()
        );
    }
    Ok(())
}
