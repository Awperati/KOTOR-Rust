//! Headless inspection tool. Reads the installed game, writes only where told.
//!
//! Set `KOTOR_DIR` to point at another install. The game folder is never written to.

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
  gff <name.ext>               print a GFF resource as text
  2da <name> [rows]            print a 2DA table
  tlk <strref>                 print a dialog.tlk string
  disasm <name>                disassemble an NCS script
  png <name> <out.png>         decode a TGA/TPC texture to PNG
  obj <model> <out.obj>        export a model's rest pose as OBJ
  render <model> <out.png> [px]  software-render a model (front view)
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
            let g = Gff::parse(&open()?.read(name, ty)?)?;
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
