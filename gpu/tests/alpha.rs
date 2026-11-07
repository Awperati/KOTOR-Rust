//! Alpha on the GPU, with made-up pictures so that the result is known: a coloured sheet in front of a green one.
use kotor_gpu::GpuRenderer;
use kotor_image::Image;
use kotor_render::{Camera, Scene, SceneMesh};

const SIZE: u32 = 64;

fn sheet(z: f32, texture: &str) -> SceneMesh {
    SceneMesh {
        shade: 0.0,
        fade: 0.0,
        name: texture.to_string(),
        positions: vec![
            [-2.0, -2.0, z],
            [2.0, -2.0, z],
            [2.0, 2.0, z],
            [-2.0, 2.0, z],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        uvs2: vec![[0.0; 2]; 4],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        texture: Some(texture.to_string()),
        lightmap: None,
    }
}

fn picture(pixels: impl Fn(u32) -> [u8; 4]) -> Image {
    let mut rgba = Vec::new();
    for _y in 0..4 {
        for x in 0..4 {
            rgba.extend(pixels(x));
        }
    }
    Image {
        width: 4,
        height: 4,
        rgba,
    }
}

fn textures(name: &str) -> Option<Image> {
    Some(match name {
        "green" => picture(|_| [0, 255, 0, 255]),
        // The left half is gone, the right half solid red.
        "cut" => picture(|x| {
            if x < 2 {
                [255, 0, 0, 0]
            } else {
                [255, 0, 0, 255]
            }
        }),
        // All of it half there.
        "soft" => picture(|_| [255, 0, 0, 128]),
        _ => return None,
    })
}

fn draw(front: &str, transparency: bool) -> Option<Image> {
    let mut gpu = GpuRenderer::new(SIZE, SIZE).ok()?;
    gpu.set_transparency(transparency);
    let scene = Scene {
        meshes: vec![sheet(-1.0, "green"), sheet(0.0, front)],
        ..Scene::default()
    };
    let cam = Camera {
        eye: [0.0, -0.001, 3.0],
        target: [0.0, 0.0, 0.0],
        fov: 60.0,
    };
    gpu.render(&scene, &mut textures, &cam).ok()
}

fn at(img: &Image, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
}

#[test]
fn a_mostly_clear_or_solid_picture_cuts_holes() {
    let Some(img) = draw("cut", true) else { return };
    let (left, right) = (
        at(&img, SIZE / 4, SIZE / 2),
        at(&img, SIZE * 3 / 4, SIZE / 2),
    );
    assert!(left[1] > 100 && left[0] < 30, "the green shows: {left:?}");
    assert!(right[0] > 100 && right[1] < 30, "the red stays: {right:?}");
}

#[test]
fn a_half_there_picture_blends_with_what_is_behind() {
    let Some(img) = draw("soft", true) else {
        return;
    };
    let c = at(&img, SIZE / 2, SIZE / 2);
    assert!(c[0] > 60 && c[1] > 60, "red and green mixed: {c:?}");
}

#[test]
fn with_transparency_off_everything_is_solid() {
    let Some(img) = draw("cut", false) else {
        return;
    };
    let left = at(&img, SIZE / 4, SIZE / 2);
    assert!(left[0] > 100 && left[1] < 30, "{left:?}");
}
