//! Headless inspection tool. Reads the installed game, writes only where told.
//!
//! Set `KOTOR_DIR` to point at another install. The game folder is never written to.

mod author;
mod render;

use kotor_archive::Install;
use kotor_core::{Error, ResType, Result};
use kotor_gff::{Gff, Struct, Value};
use std::collections::BTreeMap;
use std::fmt::Write as _;

const USAGE: &str = "\
kotor-tool <command> [args]
  info                         install summary
  module <name>                list a module's resources and layout rooms
  cat <name.ext> <out>         write one resource's raw bytes to a file
  gff <name.ext> [module]      print a GFF resource as text (module = layer that module first)
  2da <name> [rows]            print a 2DA table
  tlk <strref>                 print a dialog.tlk string
  disasm <name>                disassemble an NCS script
  png <name> <out.png>         decode a TGA/TPC texture to PNG
  obj <model> <out.obj>        export a model's rest pose as OBJ
  view <area> <out.png> [--module M] [--eye x y z] [--target x y z] [--size WxH] [--fov deg]
                               software-render an area's rooms with textures and lightmaps
  creature <template> <out.png> [--module M] [--anim name] [--time s]
                               draw a creature template via appearance.2da (body + head, texture variation)
  character <body> <out.png> [--head H] [--anim name] [--time s] [--size WxH]
                               posed character (body + head) with textures, software-rendered
  render <model> <out.png> [px]  software-render a model (front view)
  author clone <module> <display name> <out.mod> [new_area|-] [--npc <id> <name> <dialogue.txt> [template]]
                               copy a module to a new .mod; with new_area it can sit beside the original
  author dlg <text> <out.dlg>  build a DLG from the dialogue text format (see kotor-gff dlg docs)
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn split(arg: &str) -> Result<(&str, ResType)> {
    let (name, ext) = arg
        .rsplit_once('.')
        .ok_or_else(|| Error::format("expected name.ext"))?;
    let ty = ResType::from_extension(ext)
        .ok_or_else(|| Error::format(format!("unknown extension {ext}")))?;
    Ok((name, ty))
}

fn need(args: &[String], i: usize) -> Result<&str> {
    args.get(i)
        .map(String::as_str)
        .ok_or_else(|| Error::format(USAGE))
}

fn open() -> Result<Install> {
    let dir = kotor_core::install_dir()
        .ok_or_else(|| Error::NotFound("KotOR install (set KOTOR_DIR)".into()))?;
    Install::open(dir)
}

fn run(args: &[String]) -> Result<()> {
    match need(args, 0)? {
        "info" => {
            let inst = open()?;
            println!("install: {}", inst.dir().display());
            println!("bif files: {}", inst.key().bifs.len());
            let mut by_type: BTreeMap<ResType, usize> = BTreeMap::new();
            for e in &inst.key().entries {
                *by_type.entry(e.restype).or_default() += 1;
            }
            for (t, n) in by_type {
                println!("  {t:>6}: {n}");
            }
        }
        "module" => {
            let name = need(args, 1)?;
            let mut inst = open()?;
            inst.load_module(name)?;
            let mut rows: Vec<(String, String, u32)> = inst
                .overlay_resources()
                .map(|(n, t, s)| (n.to_string(), t.to_string(), s))
                .collect();
            rows.sort();
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for (_, t, _) in &rows {
                *counts.entry(t).or_default() += 1;
            }
            println!("module {name}: {} resources {counts:?}", rows.len());
            for (n, t, s) in &rows {
                println!("  {n}.{t}  {s} bytes");
            }
            let area = inst
                .read("module", ResType::IFO)
                .ok()
                .and_then(|b| Gff::parse(&b).ok())
                .and_then(|g| g.root.str("Mod_Entry_Area").map(str::to_string))
                .unwrap_or_else(|| name.to_string());
            println!("entry area: {area}");
            if let Ok(bytes) = inst.read(&area, ResType::LYT) {
                let lyt = kotor_walkmesh::parse_lyt(&bytes)?;
                println!("layout: {} rooms", lyt.rooms.len());
                for r in &lyt.rooms {
                    println!("  {} at {:?}", r.model, r.position);
                }
            }
        }
        "cat" => {
            let (name, ty) = split(need(args, 1)?)?;
            let mut inst = open()?;
            inst.load_texture_packs()?;
            std::fs::write(need(args, 2)?, inst.read(name, ty)?)?;
        }
        "gff" => {
            let (name, ty) = split(need(args, 1)?)?;
            let mut inst = open()?;
            // Optional third argument: a module to layer first (resources inside modules).
            if let Some(m) = args.get(2) {
                inst.load_module(m)?;
            }
            let g = Gff::parse(&inst.read(name, ty)?)?;
            let mut s = String::new();
            let _ = writeln!(s, "{} (root struct id {})", g.file_type, g.root.id);
            dump_struct(&g.root, 1, &mut s);
            print!("{s}");
        }
        "2da" => {
            let t = kotor_tables::TwoDa::parse(&open()?.read(need(args, 1)?, ResType::TWODA)?)?;
            let limit: usize = args
                .get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(usize::MAX);
            println!("{}", t.columns.join("\t"));
            for (l, r) in t.labels.iter().zip(&t.rows).take(limit) {
                println!("{l}\t{}", r.join("\t"));
            }
        }
        "tlk" => {
            let inst = open()?;
            let tlk = kotor_tables::Tlk::parse(&std::fs::read(inst.dir().join("dialog.tlk"))?)?;
            let id: u32 = need(args, 1)?
                .parse()
                .map_err(|_| Error::format("strref must be a number"))?;
            println!(
                "{}",
                tlk.get(id)
                    .ok_or_else(|| Error::NotFound(format!("strref {id}")))?
            );
        }
        "disasm" => {
            let p = kotor_script::disassemble(&open()?.read(need(args, 1)?, ResType::NCS)?)?;
            print!("{}", p.listing());
        }
        "png" => {
            let name = need(args, 1)?;
            let mut inst = open()?;
            inst.load_texture_packs()?;
            let img = if inst.contains(name, ResType::TPC) {
                kotor_image::parse_tpc(&inst.read(name, ResType::TPC)?)?.image
            } else {
                kotor_image::parse_tga(&inst.read(name, ResType::TGA)?)?
            };
            std::fs::write(need(args, 2)?, kotor_image::to_png(&img))?;
        }
        "view" => {
            // view <area> <out.png> [--module M] [--eye x y z] [--target x y z] [--size WxH] [--fov deg]
            let area = need(args, 1)?;
            let out = need(args, 2)?;
            let flag = |name: &str, n: usize| -> Option<Vec<f32>> {
                let i = args.iter().position(|a| a == name)?;
                (1..=n).map(|k| args.get(i + k)?.parse().ok()).collect()
            };
            let mut inst = open()?;
            if let Some(i) = args.iter().position(|a| a == "--module") {
                inst.load_module(need(args, i + 1)?)?;
            }
            inst.load_texture_packs()?;
            let mut scene = kotor_render::load_area(&mut inst, area)?;
            let stats = kotor_render::add_area_objects(&mut scene, &mut inst, area)?;
            println!("{stats:?}");
            let mid = [0, 1, 2].map(|k| (scene.min[k] + scene.max[k]) / 2.0);
            let eye = flag("--eye", 3).map_or([mid[0], scene.min[1] - 2.0, mid[2] + 6.0], |v| {
                [v[0], v[1], v[2]]
            });
            let target = flag("--target", 3).map_or(mid, |v| [v[0], v[1], v[2]]);
            let (w, h) = args
                .iter()
                .position(|a| a == "--size")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.split_once('x'))
                .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                .unwrap_or((960u32, 540u32));
            let fov = flag("--fov", 1).map_or(70.0, |v| v[0]);
            let cam = kotor_render::Camera { eye, target, fov };
            let t0 = std::time::Instant::now();
            let img = kotor_render::render(
                &scene,
                &mut |n| kotor_render::load_texture(&mut inst, n),
                &cam,
                w,
                h,
            );
            println!(
                "{} meshes, {} triangles, bounds {:?}..{:?}, {w}x{h} in {:.1}s",
                scene.meshes.len(),
                scene.triangle_count(),
                scene.min,
                scene.max,
                t0.elapsed().as_secs_f32()
            );
            std::fs::write(out, kotor_image::to_png(&img))?;
        }
        "creature" => {
            // creature <template> <out.png> [--module M] [--anim name] [--time s] [--size WxH]
            let tpl = need(args, 1)?;
            let out = need(args, 2)?;
            let opt = |name: &str| -> Option<&str> {
                let i = args.iter().position(|a| a == name)?;
                args.get(i + 1).map(String::as_str)
            };
            let mut inst = open()?;
            if let Some(m) = opt("--module") {
                inst.load_module(m)?;
            }
            inst.load_texture_packs()?;
            // A number means "appearance.2da row" (texture variation 1); a name is a creature template.
            let (row, var) = match tpl.parse::<usize>() {
                Ok(row) => (row, 1),
                Err(_) => {
                    let utc = Gff::parse(&inst.read(tpl, ResType(2027))?)?;
                    let row = utc
                        .root
                        .int("Appearance_Type")
                        .ok_or_else(|| Error::format("template has no Appearance_Type"))?
                        as usize;
                    (row, utc.root.int("TextureVar").unwrap_or(1) as u32)
                }
            };
            let time: f32 = opt("--time").and_then(|t| t.parse().ok()).unwrap_or(0.0);
            let anim = opt("--anim").map(|a| (a, time));
            let scene = kotor_render::load_creature(&mut inst, row, var, anim)?;
            let (w, h) = opt("--size")
                .and_then(|s| s.split_once('x'))
                .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                .unwrap_or((360u32, 480u32));
            let mid = [0, 1, 2].map(|k| (scene.min[k] + scene.max[k]) / 2.0);
            let height = scene.max[2] - scene.min[2];
            let cam = kotor_render::Camera {
                eye: [mid[0], mid[1] + height * 2.2, mid[2] + height * 0.1],
                target: mid,
                fov: 40.0,
            };
            let img = kotor_render::render(
                &scene,
                &mut |n| kotor_render::load_texture(&mut inst, n),
                &cam,
                w,
                h,
            );
            println!(
                "appearance row {row}, texture variation {var}: {} meshes, height {:.2} m",
                scene.meshes.len(),
                height
            );
            std::fs::write(out, kotor_image::to_png(&img))?;
        }
        "character" => {
            // character <body> <out.png> [--head H] [--anim name] [--time s] [--size WxH]
            let body = need(args, 1)?;
            let out = need(args, 2)?;
            let opt = |name: &str| -> Option<&str> {
                let i = args.iter().position(|a| a == name)?;
                args.get(i + 1).map(String::as_str)
            };
            let time: f32 = opt("--time").and_then(|t| t.parse().ok()).unwrap_or(0.0);
            let mut inst = open()?;
            inst.load_texture_packs()?;
            let anim = opt("--anim").map(|a| (a, time));
            let scene = kotor_render::load_character(&mut inst, body, opt("--head"), anim)?;
            let (w, h) = opt("--size")
                .and_then(|s| s.split_once('x'))
                .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                .unwrap_or((480u32, 640u32));
            let mid = [0, 1, 2].map(|k| (scene.min[k] + scene.max[k]) / 2.0);
            let height = scene.max[2] - scene.min[2];
            let cam = kotor_render::Camera {
                eye: [mid[0], mid[1] + height * 2.2, mid[2] + height * 0.1],
                target: mid,
                fov: 40.0,
            };
            let img = kotor_render::render(
                &scene,
                &mut |n| kotor_render::load_texture(&mut inst, n),
                &cam,
                w,
                h,
            );
            println!(
                "{} meshes, {} triangles, height {:.2} m",
                scene.meshes.len(),
                scene.triangle_count(),
                height
            );
            std::fs::write(out, kotor_image::to_png(&img))?;
        }
        "obj" | "render" => {
            let name = need(args, 1)?;
            let mut inst = open()?;
            let mdl = inst.read(name, ResType::MDL)?;
            let mdx = inst.read(name, ResType::MDX).unwrap_or_default();
            let model = kotor_model::parse_model(&mdl, &mdx)?;
            if args[0] == "obj" {
                std::fs::write(need(args, 2)?, kotor_model::to_obj(&model))?;
            } else {
                let px = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(512);
                let img = render::render(&model, px);
                std::fs::write(need(args, 2)?, kotor_image::to_png(&img))?;
            }
        }
        "author" => match need(args, 1)? {
            "clone" => {
                // author clone <module> <display name> <out.mod> [new_area]
                let mut inst = open()?;
                let out = std::path::Path::new(need(args, 4)?);
                author::check_output_path(&inst, out)?;
                // Optional 5th arg: new area name ("-" keeps the original), then
                // `--npc <id> <name> <dialogue.txt> [template]`.
                let area = args
                    .get(5)
                    .map(String::as_str)
                    .filter(|a| *a != "-" && !a.starts_with("--"));
                let npc = match args.iter().position(|a| a == "--npc") {
                    Some(i) => Some(author::NpcSpec {
                        id: need(args, i + 1)?.to_string(),
                        name: need(args, i + 2)?.to_string(),
                        dialogue: std::fs::read_to_string(need(args, i + 3)?)?,
                        template: args
                            .get(i + 4)
                            .cloned()
                            .unwrap_or_else(|| "end_trask".into()),
                    }),
                    None => None,
                };
                let c = author::clone_module(
                    &mut inst,
                    need(args, 2)?,
                    need(args, 3)?,
                    area,
                    npc.as_ref(),
                )?;
                std::fs::write(out, &c.bytes)?;
                for r in &c.renamed {
                    println!("renamed {r}");
                }
                println!(
                    "{} string fields rewritten, {} bytes written",
                    c.string_edits,
                    c.bytes.len()
                );
            }
            "dlg" => {
                // author dlg <text file> <out.dlg>
                let inst = open()?;
                let out = std::path::Path::new(need(args, 3)?);
                author::check_output_path(&inst, out)?;
                let text = std::fs::read_to_string(need(args, 2)?)?;
                std::fs::write(out, author::dialog_from_text(&text)?)?;
            }
            _ => return Err(Error::format(USAGE)),
        },
        _ => return Err(Error::format(USAGE)),
    }
    Ok(())
}

fn dump_struct(s: &Struct, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    for (label, v) in &s.fields {
        match v {
            Value::Struct(c) => {
                let _ = writeln!(out, "{pad}{label}: struct #{}", c.id);
                dump_struct(c, depth + 1, out);
            }
            Value::List(items) => {
                let _ = writeln!(out, "{pad}{label}: list[{}]", items.len());
                for (i, c) in items.iter().enumerate() {
                    let _ = writeln!(out, "{pad}  [{i}] struct #{}", c.id);
                    dump_struct(c, depth + 2, out);
                }
            }
            Value::LocString(l) => {
                let text = l.strings.first().map(|(_, t)| t.as_str()).unwrap_or("");
                let _ = writeln!(out, "{pad}{label}: strref {} {text:?}", l.strref);
            }
            Value::Void(b) => {
                let _ = writeln!(out, "{pad}{label}: <{} bytes>", b.len());
            }
            other => {
                let _ = writeln!(out, "{pad}{label}: {other:?}");
            }
        }
    }
}
