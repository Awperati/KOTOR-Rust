//! Content authoring helpers. Output goes only where the caller says; never into the install.

use kotor_archive::{write_archive, ErfKind, Install, NewResource};
use kotor_core::{Error, ResType, Result};
use kotor_gff::dlg::Dialog;
use kotor_gff::{Gff, LocString, Struct, Value};
use std::path::Path;

/// Result of cloning a module.
pub struct Cloned {
    pub bytes: Vec<u8>,
    /// `old.ext -> new.ext` for every resource that was renamed or copied under the new area name.
    pub renamed: Vec<String>,
    /// Number of string fields rewritten inside IFO/ARE/GIT.
    pub string_edits: usize,
}

/// Replace every string or resref equal to `old` (case-insensitive) with `new`.
fn replace_strings(s: &mut Struct, old: &str, new: &str) -> usize {
    let mut n = 0;
    for (_, v) in &mut s.fields {
        match v {
            Value::String(t) | Value::ResRef(t) if t.eq_ignore_ascii_case(old) => {
                *t = new.to_string();
                n += 1;
            }
            Value::Struct(c) => n += replace_strings(c, old, new),
            Value::List(l) => {
                n += l
                    .iter_mut()
                    .map(|c| replace_strings(c, old, new))
                    .sum::<usize>()
            }
            _ => {}
        }
    }
    n
}

fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 16
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Build a `.mod` that holds every resource of module `src` with `Mod_Name` set to `display_name`.
///
/// With `new_area`, the entry area is renamed too so the copy can sit beside the original:
/// resources named after the area are renamed, its LYT/VIS are copied from the base data under
/// the new name, and IFO/ARE/GIT strings that name the area are rewritten. Room models and
/// scripts keep their original names, and compiled scripts that mention the old area by name
/// are not touched.
pub fn clone_module(
    inst: &mut Install,
    src: &str,
    display_name: &str,
    new_area: Option<&str>,
    npc: Option<&NpcSpec>,
) -> Result<Cloned> {
    inst.load_module(src)?;
    let area = {
        let ifo = Gff::parse(&inst.read("module", ResType::IFO)?)?;
        ifo.root
            .str("Mod_Entry_Area")
            .ok_or_else(|| Error::format("module.ifo has no Mod_Entry_Area"))?
            .to_ascii_lowercase()
    };
    if let Some(new) = new_area {
        if !valid_name(new) {
            return Err(Error::format(
                "new area name must be 1-16 chars of a-z, 0-9 or _ (lower case)",
            ));
        }
        if new == area || inst.contains(new, ResType::LYT) || inst.contains(new, ResType::ARE) {
            return Err(Error::format(format!(
                "area name '{new}' is already in use"
            )));
        }
    }
    let npc_prep = npc.map(|n| prepare_npc(inst, &area, n)).transpose()?;
    let listing: Vec<(String, ResType)> = inst
        .overlay_resources()
        .map(|(n, t, _)| (n.to_string(), t))
        .collect();
    let mut resources = Vec::with_capacity(listing.len() + 2);
    let (mut renamed, mut string_edits) = (Vec::new(), 0);
    for (name, ty) in listing {
        let mut data = inst.read(&name, ty)?;
        let is_area_res = name.eq_ignore_ascii_case(&area);
        let is_gff = ty == ResType::IFO || ty == ResType::ARE || ty == ResType::GIT;
        if ty == ResType::IFO && name == "module" {
            let mut g = Gff::parse(&data)?;
            g.root.set(
                "Mod_Name",
                Value::LocString(LocString {
                    strref: -1,
                    strings: vec![(0, display_name.to_string())],
                }),
            );
            if let Some(new) = new_area {
                string_edits += replace_strings(&mut g.root, &area, new);
            }
            data = g.to_bytes()?;
        } else if let (Some(new), true) = (new_area, is_gff) {
            let mut g = Gff::parse(&data)?;
            string_edits += replace_strings(&mut g.root, &area, new);
            data = g.to_bytes()?;
        }
        if let (Some(p), true) = (&npc_prep, ty == ResType::GIT && is_area_res) {
            let mut g = Gff::parse(&data)?;
            add_creature(&mut g.root, &p.id, p.spot);
            data = g.to_bytes()?;
        }
        let out_name = match new_area {
            Some(new) if is_area_res => {
                renamed.push(format!("{name}.{ty} -> {new}.{ty}"));
                new.to_string()
            }
            _ => name,
        };
        resources.push(NewResource::new(&out_name, ty, data));
    }
    if let Some(p) = npc_prep {
        renamed.push(format!(
            "added creature {} at ({:.2}, {:.2}, {:.2}) talking {}.dlg",
            p.id, p.spot[0], p.spot[1], p.spot[2], p.id
        ));
        resources.push(NewResource::new(&p.id, ResType(2027), p.utc));
        resources.push(NewResource::new(&p.id, ResType::DLG, p.dlg));
    }
    if let Some(new) = new_area {
        for ty in [ResType::LYT, ResType::VIS] {
            if let Ok(data) = inst.read(&area, ty) {
                renamed.push(format!("{area}.{ty} -> {new}.{ty} (copied from base data)"));
                resources.push(NewResource::new(new, ty, data));
            }
        }
    }
    Ok(Cloned {
        bytes: write_archive(ErfKind::Mod, &resources)?,
        renamed,
        string_edits,
    })
}

/// An authored talking NPC to add to a cloned module.
pub struct NpcSpec {
    /// Template resref, tag and dialogue resref (1-16 chars, a-z 0-9 _).
    pub id: String,
    pub name: String,
    /// Dialogue in the kotor-gff text format.
    pub dialogue: String,
    /// Existing creature template to copy appearance, stats and stock AI scripts from.
    pub template: String,
}

struct NpcPrep {
    id: String,
    utc: Vec<u8>,
    dlg: Vec<u8>,
    spot: [f32; 3],
}

/// Floor position near the module entry point: a few offsets are tried and the first one
/// that has walkable floor at about the entry height in some room of the area is used.
fn find_spot(inst: &mut Install, area: &str, entry: [f32; 3]) -> Result<[f32; 3]> {
    let lyt = kotor_walkmesh::parse_lyt(&inst.read(area, ResType::LYT)?)?;
    // Area walkmesh vertices are already in world space (checked at the Endar Spire entry
    // point), so the room offsets from the layout are not applied.
    let walkmeshes: Vec<_> = lyt
        .rooms
        .iter()
        .filter_map(|r| kotor_walkmesh::parse_bwm(&inst.read(&r.model, ResType::WOK).ok()?).ok())
        .collect();
    for (dx, dy) in [
        (3.0, 0.0),
        (-3.0, 0.0),
        (0.0, 3.0),
        (0.0, -3.0),
        (2.0, 2.0),
        (4.0, 0.0),
    ] {
        let (x, y) = (entry[0] + dx, entry[1] + dy);
        for bwm in &walkmeshes {
            if let Some(z) = bwm.height_at(x, y) {
                if (z - entry[2]).abs() < 0.6 {
                    return Ok([x, y, z]);
                }
            }
        }
    }
    Err(Error::format(
        "no walkable floor found near the module entry point",
    ))
}

fn prepare_npc(inst: &mut Install, area: &str, n: &NpcSpec) -> Result<NpcPrep> {
    if !valid_name(&n.id) {
        return Err(Error::format(
            "npc id must be 1-16 chars of a-z, 0-9 or _ (lower case)",
        ));
    }
    if inst.contains(&n.id, ResType(2027)) || inst.contains(&n.id, ResType::DLG) {
        return Err(Error::format(format!(
            "npc id {} is already used by a creature or dialogue",
            n.id
        )));
    }
    let mut g = Gff::parse(&inst.read(&n.template, ResType(2027))?)?;
    if g.file_type != "UTC " {
        return Err(Error::format("npc template is not a creature"));
    }
    let name = Value::LocString(LocString {
        strref: -1,
        strings: vec![(0, n.name.clone())],
    });
    for (label, v) in [
        ("TemplateResRef", Value::ResRef(n.id.clone())),
        ("Tag", Value::String(n.id.clone())),
        ("Conversation", Value::ResRef(n.id.clone())),
        ("FirstName", name),
        (
            "LastName",
            Value::LocString(LocString {
                strref: -1,
                strings: vec![],
            }),
        ),
        // A talking NPC must survive stray fights and must not run the template's story
        // scripts (spawn, death, user-defined).
        ("Plot", Value::Byte(1)),
        ("ScriptSpawn", Value::ResRef(String::new())),
        ("ScriptDeath", Value::ResRef(String::new())),
        ("ScriptUserDefine", Value::ResRef(String::new())),
    ] {
        g.root.set(label, v);
    }
    let utc = g.to_bytes()?;
    let dlg = dialog_from_text(&n.dialogue)?;

    let ifo = Gff::parse(&inst.read("module", ResType::IFO)?)?;
    let f = |l: &str| ifo.root.float(l).unwrap_or(0.0) as f32;
    let spot = find_spot(
        inst,
        area,
        [f("Mod_Entry_X"), f("Mod_Entry_Y"), f("Mod_Entry_Z")],
    )?;
    Ok(NpcPrep {
        id: n.id.clone(),
        utc,
        dlg,
        spot,
    })
}

/// Append a creature placement (facing -x, towards a player who enters from the west) to a GIT.
fn add_creature(git: &mut Struct, template: &str, spot: [f32; 3]) {
    let entry = Struct {
        id: 4,
        fields: vec![
            ("XPosition".into(), Value::Float(spot[0])),
            ("YPosition".into(), Value::Float(spot[1])),
            ("ZPosition".into(), Value::Float(spot[2])),
            ("XOrientation".into(), Value::Float(-1.0)),
            ("YOrientation".into(), Value::Float(0.0)),
            ("TemplateResRef".into(), Value::ResRef(template.to_string())),
        ],
    };
    match git.fields.iter_mut().find(|(l, _)| l == "Creature List") {
        Some((_, Value::List(l))) => l.push(entry),
        _ => git
            .fields
            .push(("Creature List".into(), Value::List(vec![entry]))),
    }
}

/// Turn a dialogue authoring text into DLG file bytes.
pub fn dialog_from_text(src: &str) -> Result<Vec<u8>> {
    Dialog::from_text(src)?.to_gff().to_bytes()
}

/// Refuse to write inside the game folder.
pub fn check_output_path(inst: &Install, out: &Path) -> Result<()> {
    let install = std::fs::canonicalize(inst.dir())?;
    // The folder may not exist yet: judge the nearest ancestor that does.
    let mut parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    while !parent.exists() {
        parent = parent
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
    }
    let parent = std::fs::canonicalize(parent)?;
    if parent.starts_with(&install) {
        return Err(Error::format(
            "refusing to write inside the game folder; choose another output folder",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kotor_archive::{Erf, Source};

    #[test]
    fn clone_keeps_resources_and_renames() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let mut inst = Install::open(&dir).unwrap();
        let c = clone_module(&mut inst, "end_m01aa", "Test Spire", None, None).unwrap();
        let new = Erf::open(Source::memory(c.bytes)).unwrap();
        assert_eq!(new.kind, ErfKind::Mod);

        let mut base = Install::open(&dir).unwrap();
        base.load_module("end_m01aa").unwrap();
        let n = base.overlay_resources().count();
        assert_eq!(new.entries.len(), n);
        let ifo = Gff::parse(&new.get("module", ResType::IFO).unwrap()).unwrap();
        assert_eq!(ifo.root.loc("Mod_Name").unwrap().strings[0].1, "Test Spire");
        assert_eq!(ifo.root.str("Mod_Entry_Area"), Some("m01aa"));
        let are = base.read("m01aa", ResType::ARE).unwrap();
        assert_eq!(new.get("m01aa", ResType::ARE).unwrap(), are);
    }

    #[test]
    fn area_rename_sits_beside_the_original() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let mut inst = Install::open(&dir).unwrap();
        let c = clone_module(&mut inst, "end_m01aa", "Spire Copy", Some("zz_spire"), None).unwrap();
        assert!(c.string_edits >= 2, "only {} edits", c.string_edits);
        let new = Erf::open(Source::memory(c.bytes)).unwrap();
        let has = |n: &str, t| new.find(&kotor_core::ResRef::new(n), t).is_some();
        assert!(has("zz_spire", ResType::ARE));
        assert!(has("zz_spire", ResType::GIT));
        assert!(has("zz_spire", ResType::LYT));
        assert!(has("zz_spire", ResType::VIS));
        assert!(!has("m01aa", ResType::ARE));
        assert!(!has("m01aa", ResType::GIT));

        // No IFO/ARE/GIT string still names the old area.
        fn mentions(s: &Struct, name: &str) -> bool {
            s.fields.iter().any(|(_, v)| match v {
                Value::String(t) | Value::ResRef(t) => t.eq_ignore_ascii_case(name),
                Value::Struct(c) => mentions(c, name),
                Value::List(l) => l.iter().any(|c| mentions(c, name)),
                _ => false,
            })
        }
        for ty in [ResType::IFO, ResType::ARE, ResType::GIT] {
            let name = if ty == ResType::IFO {
                "module"
            } else {
                "zz_spire"
            };
            let g = Gff::parse(&new.get(name, ty).unwrap()).unwrap();
            assert!(!mentions(&g.root, "m01aa"), "{ty} still names m01aa");
        }
        let ifo = Gff::parse(&new.get("module", ResType::IFO).unwrap()).unwrap();
        assert_eq!(ifo.root.str("Mod_Entry_Area"), Some("zz_spire"));
        // The layout is a byte copy of the original's.
        let lyt = inst.read("m01aa", ResType::LYT).unwrap();
        assert_eq!(new.get("zz_spire", ResType::LYT).unwrap(), lyt);
    }

    #[test]
    fn bad_or_taken_area_names_are_refused() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        for bad in [
            "m01aa",
            "M01AA",
            "has space",
            "this_is_far_too_long_a_name",
            "m02aa",
        ] {
            let mut inst = Install::open(&dir).unwrap();
            assert!(
                clone_module(&mut inst, "end_m01aa", "x", Some(bad), None).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn talking_npc_is_added_on_walkable_floor() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let mut inst = Install::open(&dir).unwrap();
        let npc = NpcSpec {
            id: "zz_guide".into(),
            name: "Guide".into(),
            dialogue: "start E0\nentry E0 speaker=Guide: Welcome.\n  link R0\nreply R0: Thanks.\n"
                .into(),
            template: "end_trask".into(),
        };
        let c = clone_module(
            &mut inst,
            "end_m01aa",
            "NPC Test",
            Some("zz_spire"),
            Some(&npc),
        )
        .unwrap();
        let new = Erf::open(Source::memory(c.bytes)).unwrap();
        let utc = Gff::parse(&new.get("zz_guide", ResType(2027)).unwrap()).unwrap();
        assert_eq!(utc.root.str("Conversation"), Some("zz_guide"));
        assert_eq!(utc.root.str("Tag"), Some("zz_guide"));
        assert_eq!(utc.root.str("ScriptSpawn"), Some(""));
        assert_eq!(utc.root.str("ScriptDialogue"), Some("k_hen_dialogue01"));
        let d = Dialog::from_gff(&Gff::parse(&new.get("zz_guide", ResType::DLG).unwrap()).unwrap())
            .unwrap();
        assert_eq!(d.entries.len(), 1);
        let git = Gff::parse(&new.get("zz_spire", ResType::GIT).unwrap()).unwrap();
        let list = git.root.list("Creature List").unwrap();
        let mine = list.last().unwrap();
        assert_eq!(mine.str("TemplateResRef"), Some("zz_guide"));
        assert_eq!(list.len(), 27, "one creature added to the original 26");
        let (x, y, z) = (
            mine.float("XPosition").unwrap() as f32,
            mine.float("YPosition").unwrap() as f32,
            mine.float("ZPosition").unwrap() as f32,
        );
        assert!(
            (z - -1.275).abs() < 0.6 && (x - 15.4248).abs() < 5.0 && (y - 20.1215).abs() < 5.0,
            "{x} {y} {z}"
        );
    }

    #[test]
    fn npc_id_collisions_are_refused() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let mut inst = Install::open(&dir).unwrap();
        let npc = NpcSpec {
            id: "end_trask".into(),
            name: "x".into(),
            dialogue: "start E0\nentry E0: hi\n".into(),
            template: "end_trask".into(),
        };
        assert!(clone_module(&mut inst, "end_m01aa", "x", None, Some(&npc)).is_err());
    }

    #[test]
    fn dialogue_text_builds_a_dlg() {
        let bytes = dialog_from_text(
            "start E0\nentry E0 speaker=Carth: Hi.\n  link R0\nreply R0: Hello.\n",
        )
        .unwrap();
        let d = Dialog::from_gff(&Gff::parse(&bytes).unwrap()).unwrap();
        assert_eq!(d.entries.len(), 1);
        assert_eq!(d.entries[0].links[0].index, 0);
    }

    #[test]
    fn output_inside_install_is_refused() {
        let Some(dir) = kotor_core::install_dir() else {
            return;
        };
        let inst = Install::open(&dir).unwrap();
        assert!(check_output_path(&inst, &dir.join("modules").join("x.mod")).is_err());
        assert!(check_output_path(&inst, &std::env::temp_dir().join("x.mod")).is_ok());
    }
}
