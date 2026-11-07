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
  disasm <name> [module]       disassemble an NCS script (module = layer that module first)
  png <name> <out.png>         decode a TGA/TPC texture to PNG
  anims <model>                list a model's animations: length and when it stops moving
  obj <model> <out.obj>        export a model's rest pose as OBJ
  view <area> <out.png> [--module M] [--eye x y z] [--target x y z] [--size WxH] [--fov deg]
                               software-render an area's rooms with textures and lightmaps
  walk <area> <out_dir> [--module M] [--frames N] [--size WxH] [--appearance row] [--path x,y x,y ...] [--goto x,y]
                               a character walks the walkmesh with a follow camera; frames + contact.png
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
    let mut inst = Install::open(dir)?;
    // KOTOR_ORIGINAL=1 reads a module's own .rim pair even when an installed mod left a .mod beside it.
    inst.set_prefer_rim(std::env::var_os("KOTOR_ORIGINAL").is_some());
    Ok(inst)
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
            let mut inst = open()?;
            if let Some(m) = args.get(2) {
                inst.load_module(m)?;
            }
            let p = kotor_script::disassemble(&inst.read(need(args, 1)?, ResType::NCS)?)?;
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
            if !args.iter().any(|a| a == "--no-creatures") {
                let cs = kotor_render::add_area_creatures(
                    &mut scene,
                    &mut inst,
                    area,
                    Some(("pause1", 0.0)),
                )?;
                println!("{cs:?}");
            }

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
        "walk" => {
            // walk <area> <out_dir> [--module M] [--frames N] [--size WxH] [--appearance row] [--path x,y x,y ...]
            let area = need(args, 1)?;
            let out_dir = std::path::Path::new(need(args, 2)?);
            let opt = |name: &str| -> Option<&str> {
                let i = args.iter().position(|a| a == name)?;
                args.get(i + 1).map(String::as_str)
            };
            let mut inst = open()?;
            author::check_output_path(&inst, &out_dir.join("x.png"))?;
            if let Some(m) = opt("--module") {
                inst.load_module(m)?;
            }
            inst.load_texture_packs()?;
            let row: usize = opt("--appearance")
                .and_then(|r| r.parse().ok())
                .unwrap_or(4);
            let frames: usize = opt("--frames").and_then(|r| r.parse().ok()).unwrap_or(24);
            let (w, h) = opt("--size")
                .and_then(|s| s.split_once('x'))
                .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                .unwrap_or((480u32, 270u32));
            // Waypoints: after --path, every argument of the form x,y (default: along the first Endar Spire corridor).
            let mut waypoints: Vec<[f32; 2]> = match args.iter().position(|a| a == "--path") {
                Some(i) => args[i + 1..]
                    .iter()
                    .take_while(|a| !a.starts_with("--"))
                    .filter_map(|p| {
                        let (x, y) = p.split_once(',')?;
                        Some([x.parse().ok()?, y.parse().ok()?])
                    })
                    .collect(),
                None => Vec::new(),
            };
            let ifo = Gff::parse(&inst.read("module", ResType::IFO)?)?;
            let entry = |l: &str| ifo.root.float(l).unwrap_or(0.0) as f32;
            let goto = opt("--goto").and_then(|g| {
                let (x, y) = g.split_once(',')?;
                Some([x.parse::<f32>().ok()?, y.parse::<f32>().ok()?])
            });
            if let (true, Some(g)) = (waypoints.is_empty(), goto) {
                // Click-to-move: route over the walkmesh from the spawn point to the clicked spot.
                let nav = kotor_render::Floor::load(&mut inst, area)?.navmesh();
                let from = [
                    entry("Mod_Entry_X"),
                    entry("Mod_Entry_Y"),
                    entry("Mod_Entry_Z"),
                ];
                let route = nav
                    .find_path(from, [g[0], g[1], from[2]])
                    .ok_or_else(|| Error::format("no route to that point"))?;
                waypoints = route.iter().map(|p| [p[0], p[1]]).collect();
                println!("route of {} points over the walkmesh", waypoints.len());
            }
            if waypoints.is_empty() {
                let (ex, ey) = (entry("Mod_Entry_X"), entry("Mod_Entry_Y"));
                waypoints = vec![[ex, ey], [ex + 15.0, ey + 0.5], [ex + 23.0, ey + 0.5]];
            }
            let floor = kotor_render::Floor::load(&mut inst, area)?;
            let steps = kotor_render::walk_path(&floor, &waypoints, entry("Mod_Entry_Z"), 0.05)?;
            let mut scene = kotor_render::load_area(&mut inst, area)?;
            // Doors are left out: the walk goes through them as if they were open.
            let stats = kotor_render::add_area_objects_with(&mut scene, &mut inst, area, false)?;
            println!("{stats:?}");
            let rig = kotor_render::rig_for_appearance(&mut inst, row, 1)?;
            let app = kotor_tables::TwoDa::parse(&inst.read("appearance", ResType::TWODA)?)?;
            let per_cycle = app.float(row, "walkdist").unwrap_or(1.6) as f32;
            let opts = kotor_render::WalkOptions {
                frames,
                width: w,
                height: h,
                animation: "walk",
                metres_per_cycle: per_cycle,
            };
            let mut cache: std::collections::HashMap<String, Option<kotor_image::Image>> =
                Default::default();
            let t0 = std::time::Instant::now();
            let images = kotor_render::render_walk(
                &scene,
                &rig,
                &steps,
                &mut |n| {
                    cache
                        .entry(n.to_string())
                        .or_insert_with(|| kotor_render::load_texture(&mut inst, n))
                        .clone()
                },
                &opts,
            )?;
            std::fs::create_dir_all(out_dir)?;
            for (i, img) in images.iter().enumerate() {
                std::fs::write(
                    out_dir.join(format!("frame_{i:03}.png")),
                    kotor_image::to_png(img),
                )?;
            }
            // Contact sheet: 8 frames spread over the walk, 4 per row.
            let n = images.len();
            let picks: Vec<kotor_image::Image> = if n <= 8 {
                images.clone()
            } else {
                (0..8).map(|k| images[k * (n - 1) / 7].clone()).collect()
            };
            if let Some(sheet) = kotor_render::contact_sheet(&picks, 4) {
                std::fs::write(out_dir.join("contact.png"), kotor_image::to_png(&sheet))?;
            }
            let total = steps.last().map_or(0.0, |s| s.distance);
            println!(
                "{} frames, {total:.1} m walked, {w}x{h}, {:.1}s",
                images.len(),
                t0.elapsed().as_secs_f32()
            );
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
        "anims" => {
            let name = need(args, 1)?;
            let mut inst = open()?;
            let mdl = inst.read(name, ResType::MDL)?;
            let mdx = inst.read(name, ResType::MDX).unwrap_or_default();
            let model = kotor_model::parse_model(&mdl, &mdx)?;
            println!("super {:?} scale {}", model.supermodel, model.anim_scale);
            for a in &model.animations {
                // When anything in the model moves, sampled every thirtieth of a second.
                let at = |t: f32| model.world_matrices(Some(&a.sample(&model, t)));
                let steps = (a.length * 30.0) as usize;
                let mut still_from = 0.0;
                let mut moving_from = None;
                let mut last = at(0.0);
                for k in 1..=steps {
                    let t = k as f32 / 30.0;
                    let now = at(t);
                    let moved = now
                        .iter()
                        .zip(&last)
                        .any(|((_, p), (_, q))| (0..3).any(|r| (p[r][3] - q[r][3]).abs() > 0.001));
                    if moved {
                        moving_from.get_or_insert(t - 1.0 / 30.0);
                        still_from = t;
                    }
                    last = now;
                }
                println!(
                    "{} {:.3} moves from {:.3} until {:.3}",
                    a.name,
                    a.length,
                    moving_from.unwrap_or(0.0),
                    still_from
                );
                for e in &a.events {
                    println!("  event {:.3} {}", e.time, e.name);
                }
            }
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
