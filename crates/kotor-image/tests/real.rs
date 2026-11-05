use kotor_archive::{Erf, Install, Source};
use kotor_core::ResType;
use kotor_image::{parse_tga, parse_tpc};
use std::collections::BTreeMap;

#[test]
fn every_base_tga_decodes() {
    let Some(dir) = kotor_core::install_dir() else { return };
    let mut inst = Install::open(dir).unwrap();
    let names = inst.list(ResType::TGA);
    assert!(names.len() > 1000);
    let mut sizes: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for n in &names {
        let img = parse_tga(&inst.read(n, ResType::TGA).unwrap()).unwrap_or_else(|e| panic!("{n}: {e}"));
        assert_eq!(img.rgba.len(), (img.width * img.height * 4) as usize);
        *sizes.entry((img.width, img.height)).or_default() += 1;
    }
    println!("tga sizes: {sizes:?}");
}

#[test]
fn every_tpc_in_texture_packs_decodes() {
    let Some(dir) = kotor_core::install_dir() else { return };
    let mut total = 0;
    let mut formats: BTreeMap<String, usize> = BTreeMap::new();
    for pack in ["swpc_tex_tpa.erf", "swpc_tex_tpb.erf", "swpc_tex_tpc.erf", "swpc_tex_gui.erf"] {
        let erf = Erf::open(Source::file(dir.join("TexturePacks").join(pack))).unwrap();
        for e in erf.entries.iter().filter(|e| e.restype == ResType::TPC) {
            let t = parse_tpc(&erf.read(e).unwrap()).unwrap_or_else(|er| panic!("{pack}/{}: {er}", e.resref));
            *formats.entry(format!("{:?} cube={}", t.format, t.cube_map)).or_default() += 1;
            total += 1;
        }
    }
    println!("tpc formats: {formats:?}");
    assert!(total > 1000, "only {total} tpc");
}

#[test]
fn export_sample_pngs() {
    let Some(dir) = kotor_core::install_dir() else { return };
    let Some(out) = std::env::var_os("KOTOR_OUT") else { return };
    let out = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&out).unwrap();
    let erf = Erf::open(Source::file(dir.join("TexturePacks").join("swpc_tex_gui.erf"))).unwrap();
    for e in erf.entries.iter().filter(|e| e.restype == ResType::TPC).take(3) {
        let t = parse_tpc(&erf.read(e).unwrap()).unwrap();
        std::fs::write(out.join(format!("{}.png", e.resref)), kotor_image::to_png(&t.image)).unwrap();
    }
}
