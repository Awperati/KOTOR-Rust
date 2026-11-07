//! Cel shading on the GPU, with a made-up body: a ball lit from one side in front of a flat wall.
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

fn draw(level: u8) -> Option<Image> {
    let mut gpu = GpuRenderer::new(SIZE, SIZE).ok()?;
    gpu.set_cel_shading(level);
    let scene = Scene {
        meshes: vec![ball()],
        ..Scene::default()
    };
    let cam = Camera {
        eye: [0.0, -5.0, 1.0],
        target: [0.0, 0.0, 0.0],
        fov: 40.0,
    };
    gpu.render(&scene, &mut textures, &cam).ok()
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

#[test]
fn cel_shading_changes_the_body_and_leaves_the_space_round_it() {
    let (Some(off), Some(subtle), Some(strong)) = (draw(0), draw(1), draw(2)) else {
        return;
    };
    // The corners are the empty sky: untouched.
    for (x, y) in [(2, 2), (SIZE - 3, 2), (2, SIZE - 3), (SIZE - 3, SIZE - 3)] {
        assert_eq!(at(&off, x, y), at(&subtle, x, y), "corner {x},{y} subtle");
        assert_eq!(at(&off, x, y), at(&strong, x, y), "corner {x},{y} strong");
    }
    let (s, t) = (differing(&off, &subtle), differing(&off, &strong));
    assert!(s > 200, "subtle changes the ball: {s} pixels");
    assert!(t > s, "strong changes more than subtle: {t} against {s}");
    assert!(differing(&subtle, &strong) > 200, "the two levels differ");
}

#[test]
fn the_outline_is_dark_and_lies_outside_the_ball() {
    let (Some(off), Some(on)) = (draw(0), draw(2)) else {
        return;
    };
    // Pixels that are the ball's edge or beyond: the clear colour in the off picture, and darker than it, or dark, in the
    // on picture: a ring.
    let mut ring = 0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (o, n) = (at(&off, x, y), at(&on, x, y));
            let clear = o == at(&off, 1, 1);
            if clear && n != o && n.iter().all(|c| *c < 60) {
                ring += 1;
            }
        }
    }
    assert!(ring > 60, "a dark line round the ball: {ring} pixels");
}

/// The ball is in another area, seen through a doorway 3 m in front of the camera.
fn through_a_doorway(level: u8, door_x: f32) -> Option<Image> {
    let mut gpu = GpuRenderer::new(SIZE, SIZE).ok()?;
    gpu.set_cel_shading(level);
    let door = kotor_gpu::Aperture {
        at: [door_x, -3.0, 0.0],
        out: std::f32::consts::FRAC_PI_2,
        half: 2.0,
    };
    gpu.set_portal(Some((&Scene::default(), door)), &mut textures);
    let ball = Scene {
        meshes: vec![ball()],
        ..Scene::default()
    };
    gpu.set_portal_moving(&ball, &mut textures);
    let cam = Camera {
        eye: [0.0, -8.0, 1.0],
        target: [0.0, 0.0, 0.0],
        fov: 40.0,
    };
    gpu.render(&Scene::default(), &mut textures, &cam).ok()
}

#[test]
fn a_figure_seen_through_a_doorway_has_its_outline_inside_the_doorway() {
    let (Some(off), Some(on), Some(aside)) = (
        through_a_doorway(0, 0.0),
        through_a_doorway(2, 0.0),
        through_a_doorway(2, 40.0),
    ) else {
        return;
    };
    let sky = at(&off, 1, 1);
    let ring = |a: &Image| {
        (0..SIZE)
            .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let (o, n) = (at(&off, *x, *y), at(a, *x, *y));
                o == sky && n != o && n.iter().all(|c| *c < 60)
            })
            .count()
    };
    assert!(
        ring(&on) > 60,
        "a dark line round the figure: {}",
        ring(&on)
    );
    // A doorway that is somewhere else shows nothing of the figure, line included.
    let bare = (0..SIZE)
        .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
        .filter(|(x, y)| at(&aside, *x, *y) != sky)
        .count();
    assert_eq!(bare, 0, "no figure and no line without the doorway's view");
}
