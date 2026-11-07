//! Anti-aliasing and motion blur on the GPU, with a made-up body: a ball in front of the empty sky.
use kotor_gpu::GpuRenderer;
use kotor_image::Image;
use kotor_render::{Camera, Scene, SceneMesh};

const SIZE: u32 = 128;

/// A ball of radius 1 at the origin: normals are the positions.
fn ball() -> SceneMesh {
    let (rings, sides) = (24usize, 32usize);
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    for r in 0..=rings {
        let a = std::f32::consts::PI * r as f32 / rings as f32;
        for s in 0..=sides {
            let b = std::f32::consts::TAU * s as f32 / sides as f32;
            positions.push([a.sin() * b.cos(), a.sin() * b.sin(), a.cos()]);
            uvs.push([s as f32 / sides as f32, r as f32 / rings as f32]);
        }
    }
    let mut triangles = Vec::new();
    for r in 0..rings {
        for s in 0..sides {
            let i = (r * (sides + 1) + s) as u32;
            let j = i + sides as u32 + 1;
            triangles.push([i, j, i + 1]);
            triangles.push([i + 1, j, j + 1]);
        }
    }
    SceneMesh {
        shade: 0.0,
        fade: 0.0,
        name: "ball".into(),
        normals: positions.clone(),
        uvs2: vec![[0.0; 2]; positions.len()],
        positions,
        uvs,
        triangles,
        texture: Some("skin".into()),
        lightmap: None,
    }
}

fn textures(name: &str) -> Option<Image> {
    (name == "skin").then(|| Image {
        width: 2,
        height: 2,
        rgba: [200u8, 160, 120, 255].repeat(4),
    })
}

fn scene() -> Scene {
    Scene {
        meshes: vec![ball()],
        ..Scene::default()
    }
}

fn cam(yaw: f32) -> Camera {
    let a = yaw.to_radians();
    Camera {
        eye: [0.0, -5.0, 1.0],
        target: [5.0 * a.sin(), -5.0 + 5.0 * a.cos(), 0.0],
        fov: 40.0,
    }
}

/// A renderer, when this machine has a graphics card to make one on.
fn gpu() -> Option<GpuRenderer> {
    GpuRenderer::new(SIZE, SIZE).ok()
}

fn still(gpu: &mut GpuRenderer, aa: u8) -> Image {
    gpu.set_anti_aliasing(aa);
    gpu.set_motion_blur(0);
    gpu.render(&scene(), &mut textures, &cam(0.0)).unwrap()
}

fn at(img: &Image, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
}

fn differing(a: &Image, b: &Image) -> usize {
    a.rgba
        .chunks(4)
        .zip(b.rgba.chunks(4))
        .filter(|(p, q)| p != q)
        .count()
}

/// The mean difference of a colour channel, 0 to 255.
fn mean_diff(a: &Image, b: &Image) -> f32 {
    let total: u64 = a
        .rgba
        .iter()
        .zip(&b.rgba)
        .map(|(p, q)| p.abs_diff(*q) as u64)
        .sum();
    total as f32 / a.rgba.len() as f32
}

#[test]
fn anti_aliasing_changes_the_edge_and_leaves_the_rest() {
    let Some(mut gpu) = gpu() else { return };
    let off = still(&mut gpu, 0);
    let clear = at(&off, 1, 1);
    // The ball's centre and its radius in pixels, from where the picture is not the clear colour.
    let mut cover = Vec::new();
    for y in 0..SIZE {
        for x in 0..SIZE {
            if at(&off, x, y) != clear {
                cover.push((x as f32, y as f32));
            }
        }
    }
    assert!(cover.len() > 500, "a ball is drawn");
    let (cx, cy) = (SIZE as f32 / 2.0, SIZE as f32 / 2.0);
    let radius = (cover.len() as f32 / std::f32::consts::PI).sqrt();
    for level in 1..=3u8 {
        let on = still(&mut gpu, level);
        for (x, y) in [(2, 2), (SIZE - 3, 2), (2, SIZE - 3), (SIZE - 3, SIZE - 3)] {
            assert_eq!(
                at(&off, x, y),
                at(&on, x, y),
                "corner {x},{y} at level {level}"
            );
        }
        let n = differing(&off, &on);
        assert!(n > 20, "level {level} smooths the edge: {n} pixels");
        // Only the edge: nothing farther than 3 pixels from the rim (the 16x pass reaches one pixel more).
        for y in 0..SIZE {
            for x in 0..SIZE {
                // Edges of the ball's own triangles may move a shade (a few levels): a change of colour is what counts.
                let (p, q) = (at(&off, x, y), at(&on, x, y));
                if p.iter().zip(&q).any(|(m, n)| m.abs_diff(*n) > 12) {
                    let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                    assert!(
                        (d - radius).abs() < 4.0,
                        "level {level}: pixel {x},{y} is {d:.1} from the middle, the rim is at {radius:.1}"
                    );
                }
            }
        }
    }
    assert!(
        differing(&still(&mut gpu, 1), &still(&mut gpu, 3)) > 0,
        "16x is not the same as 4x"
    );
}

#[test]
fn the_sample_count_follows_the_level_and_the_card() {
    let Some(mut gpu) = gpu() else { return };
    let max = gpu.max_samples();
    assert!(max >= 1);
    gpu.set_anti_aliasing(0);
    assert_eq!(gpu.samples(), 1);
    gpu.set_anti_aliasing(1);
    assert_eq!(gpu.samples(), 4.min(max));
    gpu.set_anti_aliasing(2);
    assert_eq!(gpu.samples(), if max >= 8 { 8 } else { 4.min(max) });
    gpu.set_anti_aliasing(9);
    assert!(gpu.samples() <= max, "a level out of range is brought in");
}

#[test]
fn cel_outlines_still_draw_with_anti_aliasing() {
    let Some(mut gpu) = gpu() else { return };
    gpu.set_cel_shading(2);
    let off = still(&mut gpu, 0);
    let on = still(&mut gpu, 2);
    let dark = |img: &Image| {
        img.rgba
            .chunks(4)
            .filter(|p| p[0] < 60 && p[1] < 60 && p[2] < 60)
            .count()
    };
    assert!(dark(&off) > 50, "the outline is there without it");
    assert!(
        dark(&on) * 2 > dark(&off),
        "and with it: {} dark pixels, against {}",
        dark(&on),
        dark(&off)
    );
}

/// Two frames of a camera turning `deg_per_s` at `fps`, the second one kept.
fn turning(gpu: &mut GpuRenderer, blur: u8, fps: f32, deg_per_s: f32) -> Image {
    let dt = 1.0 / fps;
    gpu.set_anti_aliasing(0);
    gpu.set_motion_blur(blur);
    gpu.set_frame_time(Some(dt));
    gpu.set_motion_blur_suspended(true);
    gpu.set_motion_blur_suspended(false);
    gpu.render(&scene(), &mut textures, &cam(0.0)).unwrap();
    gpu.render(&scene(), &mut textures, &cam(deg_per_s * dt))
        .unwrap()
}

#[test]
fn motion_blur_grows_with_the_level_and_needs_movement() {
    let Some(mut gpu) = gpu() else { return };
    let sharp = turning(&mut gpu, 0, 60.0, 90.0);
    let levels: Vec<f32> = (1..=3u8)
        .map(|l| mean_diff(&sharp, &turning(&mut gpu, l, 60.0, 90.0)))
        .collect();
    assert!(levels[0] > 0.2, "subtle blurs: {}", levels[0]);
    assert!(levels[1] > levels[0] && levels[2] > levels[1], "{levels:?}");
    // A camera that does not move is not blurred, at any level.
    for l in 1..=3u8 {
        let still_cam = turning(&mut gpu, l, 60.0, 0.0);
        let reference = turning(&mut gpu, 0, 60.0, 0.0);
        assert_eq!(
            differing(&still_cam, &reference),
            0,
            "level {l} on a still camera"
        );
    }
}

#[test]
fn motion_blur_is_the_same_at_30_and_144_frames_a_second() {
    let Some(mut gpu) = gpu() else { return };
    let mut diffs = Vec::new();
    for fps in [30.0f32, 144.0] {
        let sharp = turning(&mut gpu, 0, fps, 60.0);
        let blurred = turning(&mut gpu, 2, fps, 60.0);
        diffs.push(mean_diff(&sharp, &blurred));
    }
    let ratio = diffs[0] / diffs[1];
    assert!(
        (0.6..1.7).contains(&ratio),
        "30 fps blurs {:.2} and 144 fps {:.2}",
        diffs[0],
        diffs[1]
    );
}

#[test]
fn no_blur_while_suspended_or_on_the_first_frame_of_a_new_area() {
    let Some(mut gpu) = gpu() else { return };
    let sharp = turning(&mut gpu, 0, 60.0, 90.0);
    // The first frame after a new area is drawn has nothing to be blurred from.
    gpu.set_anti_aliasing(0);
    gpu.set_motion_blur(3);
    gpu.set_frame_time(Some(1.0 / 60.0));
    gpu.set_motion_blur_suspended(false);
    gpu.render(&scene(), &mut textures, &cam(0.0)).unwrap();
    gpu.set_static_scene(&Scene::default(), &mut textures);
    let first = gpu.render(&scene(), &mut textures, &cam(1.5)).unwrap();
    assert_eq!(differing(&sharp, &first), 0, "a cut is not a smear");
    // Suspended: a menu, a loading screen, a paused game.
    gpu.render(&scene(), &mut textures, &cam(0.0)).unwrap();
    gpu.set_motion_blur_suspended(true);
    let held = gpu.render(&scene(), &mut textures, &cam(1.5)).unwrap();
    assert_eq!(differing(&sharp, &held), 0, "suspended");
}
