//! A census of every way the game changes module: what the stage can load ahead, and what still freezes.
//!
//!   cargo run --release --example census            (all modules; writes census.tsv in the current folder)
//!   cargo run --release --example census -- tar_m10aa tar_m02aa
//!
//! Reads each module's objects and compiled scripts, and lists every call to `StartNewModule` (routine 509) with who runs
//! it: a door or trigger that links (`LinkedToModule`), a conversation's end or line scripts, or a script slot of an object,
//! the area or the module. A script that runs another script by name (any text that names a script) is followed 3 deep.
//! Row columns: module, how, object tag, script, goes to, x, y, z, loaded ahead today.
//!
//! **Assumed:** the module name is the text constant nearest before the call, as `quest_route` has it; a script that builds
//! the name by code is not seen.
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_gff::{Gff, Struct, Value};
use kotor_script::{disassemble, Op};
use std::collections::{BTreeMap, BTreeSet};

const NEW_MODULE: u16 = 509;

struct Row {
    module: String,
    how: String,
    tag: String,
    script: String,
    to: String,
    at: [f32; 3],
}

/// The modules a script sends the party to (and the scripts it runs), `depth` levels down.
fn goes_to(
    inst: &mut Install,
    script: &str,
    depth: u32,
    seen: &mut BTreeSet<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    if script.is_empty() || !seen.insert(script.to_ascii_lowercase()) {
        return out;
    }
    let Ok(code) = inst.read(script, ResType::NCS) else {
        return out;
    };
    let Ok(program) = disassemble(&code) else {
        return out;
    };
    let ops: Vec<&Op> = program.instructions.iter().map(|i| &i.op).collect();
    for (at, op) in ops.iter().enumerate() {
        match op {
            Op::Action { routine, .. } if *routine == NEW_MODULE => {
                let text = ops[at.saturating_sub(10)..at]
                    .iter()
                    .rev()
                    .find_map(|o| match o {
                        Op::ConstS(s) if !s.is_empty() => Some(s.clone()),
                        _ => None,
                    });
                out.extend(text);
            }
            Op::ConstS(s) if depth > 0 && s.len() <= 16 && !s.contains(' ') => {
                let s = s.clone();
                if inst.read(&s, ResType::NCS).is_ok() {
                    out.extend(goes_to(inst, &s, depth - 1, seen));
                }
            }
            _ => {}
        }
    }
    out
}

fn position(s: &Struct) -> [f32; 3] {
    let f = |n: &str| s.float(n).unwrap_or(0.0) as f32;
    [f("XPosition"), f("YPosition"), f("ZPosition")]
}

fn main() {
    let dir = kotor_core::install_dir().expect("no KotOR install found");
    let mut wanted: Vec<String> = std::env::args().skip(1).collect();
    // `script:<name>`: what one script (and the scripts it runs) sends the party to.
    if let Some(name) = wanted.first().and_then(|a| a.strip_prefix("script:")) {
        let mut inst = Install::open(&dir).expect("install");
        // `script:<name>@<module>` reads it with that module's files loaded.
        let (name, module) = name.split_once('@').unwrap_or((name, ""));
        if !module.is_empty() {
            inst.load_module(module).expect("module");
        }
        if std::env::var_os("DUMP").is_some() {
            if let Ok(p) = inst.read(name, ResType::NCS).and_then(|b| disassemble(&b)) {
                for i in &p.instructions {
                    println!("{:5} {:?}", i.offset, i.op);
                }
            }
        }
        let mut seen = BTreeSet::new();
        let to = goes_to(&mut inst, name, 3, &mut seen);
        println!("{name} -> {to:?} (scripts read: {seen:?})");
        return;
    }
    if wanted.is_empty() {
        let mut names = BTreeSet::new();
        for e in std::fs::read_dir(dir.join("modules")).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            if let Some(s) = n.strip_suffix(".rim").or_else(|| n.strip_suffix(".mod")) {
                if !s.ends_with("_s") {
                    names.insert(s.to_string());
                }
            }
        }
        wanted = names.into_iter().collect();
    }
    let known = |m: &str| {
        ["rim", "mod"]
            .iter()
            .any(|e| dir.join("modules").join(format!("{m}.{e}")).is_file())
    };
    let mut rows: Vec<Row> = Vec::new();
    for module in &wanted {
        let Ok(mut inst) = Install::open(&dir) else {
            continue;
        };
        if inst.load_module(module).is_err() {
            continue;
        }
        let Some(ifo) = inst
            .read("module", ResType::IFO)
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
        else {
            continue;
        };
        let area = ifo
            .root
            .str("Mod_Entry_Area")
            .unwrap_or_default()
            .to_string();
        let mut found: Vec<(String, String, String, [f32; 3])> = Vec::new();
        // The module's own scripts, and the area's.
        for (name, v) in &ifo.root.fields {
            if let (true, Value::ResRef(s)) = (name.starts_with("Mod_On"), v) {
                found.push((
                    format!("module:{name}"),
                    String::new(),
                    s.to_string(),
                    [0.0; 3],
                ));
            }
        }
        if let Some(are) = inst
            .read(&area, ResType::ARE)
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
        {
            for (name, v) in &are.root.fields {
                if let (true, Value::ResRef(s)) = (name.starts_with("On"), v) {
                    found.push((
                        format!("area:{name}"),
                        String::new(),
                        s.to_string(),
                        [0.0; 3],
                    ));
                }
            }
        }
        let Some(git) = inst
            .read(&area, ResType::GIT)
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
        else {
            continue;
        };
        let lists: [(&str, &str, ResType); 6] = [
            ("Creature List", "creature", ResType(2027)),
            ("Door List", "door", ResType(2042)),
            ("Placeable List", "placeable", ResType(2044)),
            ("TriggerList", "trigger", ResType(2032)),
            ("WaypointList", "waypoint", ResType(2058)),
            ("StoreList", "store", ResType(2051)),
        ];
        let mut talks: BTreeMap<String, String> = BTreeMap::new();
        for (label, kind, ty) in lists {
            for s in git.root.list(label).unwrap_or_default() {
                let tpl = s.str("TemplateResRef").unwrap_or_default().to_string();
                let utx = inst.read(&tpl, ty).ok().and_then(|b| Gff::parse(&b).ok());
                let from: Vec<&Struct> = std::iter::once(s)
                    .chain(utx.as_ref().map(|g| &g.root))
                    .collect();
                let field = |n: &str| {
                    from.iter()
                        .find_map(|x| x.str(n).filter(|v| !v.is_empty()))
                        .unwrap_or_default()
                        .to_string()
                };
                let (tag, at) = (field("Tag"), position(s));
                let link = field("LinkedToModule");
                if !link.is_empty() {
                    rows.push(Row {
                        module: module.clone(),
                        how: format!("{kind}_link"),
                        tag: tag.clone(),
                        script: String::new(),
                        to: link.to_ascii_lowercase(),
                        at,
                    });
                }
                let talk = field("Conversation");
                if !talk.is_empty() {
                    talks
                        .entry(talk.to_ascii_lowercase())
                        .or_insert(tag.clone());
                }
                let mut scripts: BTreeMap<String, String> = BTreeMap::new();
                for x in &from {
                    for (name, v) in &x.fields {
                        if let Value::ResRef(r) = v {
                            let r = r.to_string();
                            if !r.is_empty()
                                && (name.starts_with("On") || name.starts_with("Script"))
                            {
                                scripts.entry(name.clone()).or_insert(r);
                            }
                        }
                    }
                }
                for (name, r) in scripts {
                    found.push((format!("{kind}:{name}"), tag.clone(), r, at));
                }
            }
        }
        for (talk, tag) in talks {
            let Some(dlg) = inst
                .read(&talk, ResType::DLG)
                .ok()
                .and_then(|b| Gff::parse(&b).ok())
            else {
                continue;
            };
            for f in ["EndConversation", "EndConverAbort"] {
                if let Some(s) = dlg.root.str(f) {
                    found.push((format!("talk:{f}"), tag.clone(), s.to_string(), [0.0; 3]));
                }
            }
            for list in ["EntryList", "ReplyList"] {
                for n in dlg.root.list(list).unwrap_or_default() {
                    if let Some(s) = n.str("Script") {
                        found.push(("talk:line".into(), tag.clone(), s.to_string(), [0.0; 3]));
                    }
                }
            }
        }
        for (how, tag, script, at) in found {
            let mut seen = BTreeSet::new();
            for to in goes_to(&mut inst, &script, 3, &mut seen) {
                rows.push(Row {
                    module: module.clone(),
                    how: how.clone(),
                    tag: tag.clone(),
                    script: script.clone(),
                    to: to.to_ascii_lowercase(),
                    at,
                });
            }
        }
    }
    rows.retain(|r| r.to != r.module.to_ascii_lowercase() && known(&r.to));
    rows.sort_by(|a, b| {
        (&a.module, &a.how, &a.tag, &a.to, &a.script)
            .cmp(&(&b.module, &b.how, &b.tag, &b.to, &b.script))
    });
    rows.dedup_by(|a, b| {
        (&a.module, &a.how, &a.tag, &a.to, &a.script)
            == (&b.module, &b.how, &b.tag, &b.to, &b.script)
    });
    // Loaded ahead today: the link of a door or trigger when the player is near, or a conversation's scripts.
    let ahead = |r: &Row| {
        !r.to.starts_with("stunt_") && (r.how.ends_with("_link") || r.how.starts_with("talk:"))
    };
    // What the stage's script probe says for a script of a door, trigger or placeable, run in its module.
    let mut worlds: BTreeMap<String, Option<kotor_game::World>> = BTreeMap::new();
    let mut probe = |r: &Row| -> String {
        let scripted = ["door:", "trigger:", "placeable:"]
            .iter()
            .any(|k| r.how.starts_with(k));
        if !scripted || r.script.is_empty() {
            return String::new();
        }
        let w = worlds.entry(r.module.clone()).or_insert_with(|| {
            let mut w = kotor_game::World::new(Install::open(&dir).ok()?).ok()?;
            w.load_module(&r.module).ok()?;
            Some(w)
        });
        match w.as_mut().and_then(|w| w.script_destination(&r.script, -1)) {
            Some(m) if m.eq_ignore_ascii_case(&r.to) => "yes".into(),
            Some(m) => format!("other:{m}"),
            None => "no".into(),
        }
    };
    let mut text =
        String::from("module\thow\ttag\tscript\tgoes_to\tx\ty\tz\tloaded_ahead\tprobe\n");
    let mut counts: BTreeMap<(String, &str), usize> = BTreeMap::new();
    for r in &rows {
        let p = probe(r);
        let ramp = r.module.starts_with("ebo_") && r.script.starts_with("k_pebn_");
        let a = if ahead(r) {
            "link or talk"
        } else if p == "yes" {
            "script probe"
        } else if ramp {
            "ramp, by planet"
        } else {
            "no"
        };
        let class = if r.how.starts_with("talk:") {
            "talk".to_string()
        } else if r.how.contains("_link") {
            r.how.clone()
        } else {
            format!("{} script", r.how.split(':').next().unwrap_or(""))
        };
        *counts.entry((class, a)).or_default() += 1;
        text += &format!(
            "{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.1}\t{:.1}\t{}\t{}\n",
            r.module, r.how, r.tag, r.script, r.to, r.at[0], r.at[1], r.at[2], a, p
        );
    }
    std::fs::write("census.tsv", &text).unwrap();
    println!(
        "{} transitions in {} modules (census.tsv)",
        rows.len(),
        wanted.len()
    );
    for ((class, a), n) in counts {
        println!("{n:5}  {class:24} loaded ahead: {a}");
    }
}
