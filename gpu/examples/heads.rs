//! Close pictures of a hero's neck through the GPU renderer, to judge how a head sits on its body.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example heads -- [out_dir] [appearance rows...]
use kotor_archive::Install;
use kotor_gpu::GpuRenderer;
use kotor_render::{rig_for_outfit, Camera};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/heads", String::as_str));
    std::fs::create_dir_all(&out)?;
    let rows: Vec<usize> = args.iter().skip(2).filter_map(|a| a.parse().ok()).collect();
    let rows = if rows.is_empty() {
        vec![92, 107, 122, 137, 152, 167]
    } else {
        rows
    };
    let mut inst = Install::open(kotor_core::install_dir().ok_or("no install")?)?;
    inst.set_prefer_rim(true);
    inst.load_texture_packs()?;
    let mut gpu = GpuRenderer::new(640, 640)?;
    gpu.set_sky_at_infinity(false);
    // The window's own textures, upscaled ones included.
    let mut stage = kotor_gpu::stage::Stage::open("end_m01aa", true)?;
    for row in rows {
        let rig = rig_for_outfit(&mut inst, row, 1, None)?;
        for anim in ["pause1", "g8r1", "run"] {
            let scene = rig.pose(Some((anim, 0.5)))?;
            // The neck, from where the top of the body and the bottom of the head meet.
            let head_tex = rig.head_texture().unwrap_or_default().to_string();
            let mut neck = [0.0f32, 0.0, f32::MAX];
            for m in scene
                .meshes
                .iter()
                .filter(|m| m.texture.as_deref() == Some(head_tex.as_str()))
            {
                for p in &m.positions {
                    if p[2] < neck[2] {
                        neck = *p;
                    }
                }
            }
            for (name, dx, dy, dz) in [
                ("front", 0.0, 1.0, 0.05),
                ("side", 1.0, 0.1, 0.05),
                ("back", 0.0, -1.0, 0.1),
                ("low", 0.5, 0.8, -0.35),
            ] {
                let cam = Camera {
                    eye: [neck[0] + dx * 0.9, neck[1] + dy * 0.9, neck[2] + dz],
                    target: [neck[0], neck[1], neck[2] + 0.05],
                    fov: 35.0,
                };
                let img = gpu.render(&scene, &mut |n| stage.texture(n), &cam)?;
                std::fs::write(
                    out.join(format!("row{row}_{anim}_{name}.png")),
                    kotor_image::to_png(&img),
                )?;
            }
        }
    }
    Ok(())
}
