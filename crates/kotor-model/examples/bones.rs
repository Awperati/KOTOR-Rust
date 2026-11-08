//! Which skeleton models share: `bones <model> [other models...]`. For each, its supermodel, its node count, and how many
//! of the first model's nodes it has too. Used for docs/CHARGEN_RACES.md.
use kotor_core::ResType;
use std::collections::BTreeSet;

fn names(n: &kotor_model::Node, out: &mut BTreeSet<String>) {
    out.insert(n.name.to_ascii_lowercase());
    for c in &n.children {
        names(c, out);
    }
}

fn main() {
    let models: Vec<String> = std::env::args().skip(1).collect();
    let mut inst = kotor_archive::Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mut first: Option<BTreeSet<String>> = None;
    for n in &models {
        let Ok(mdl) = inst.read(n, ResType::MDL) else {
            println!("{n}: not in the install");
            continue;
        };
        let mdx = inst.read(n, ResType::MDX).unwrap_or_default();
        let m = match kotor_model::parse_model(&mdl, &mdx) {
            Ok(m) => m,
            Err(e) => {
                println!("{n}: {e}");
                continue;
            }
        };
        let mut set = BTreeSet::new();
        names(&m.root, &mut set);
        let shared = first
            .as_ref()
            .map_or(set.len(), |f| f.intersection(&set).count());
        let face = set
            .iter()
            .filter(|s| s.starts_with("f_") && s.ends_with("_g"))
            .count();
        println!(
            "{n}: super {:?}, {} nodes, {shared} shared with {}, {face} face points, has head node: {}",
            m.supermodel,
            set.len(),
            models[0],
            set.contains("head") || set.contains("headhook")
        );
        if std::env::var_os("BONES_TEXTURES").is_some() {
            fn textures(n: &kotor_model::Node, out: &mut BTreeSet<String>) {
                if let Some(t) = n.mesh.as_ref().and_then(|m| m.texture.clone()) {
                    out.insert(format!("{}={t}", n.name));
                }
                for c in &n.children {
                    textures(c, out);
                }
            }
            let mut t = BTreeSet::new();
            textures(&m.root, &mut t);
            println!("  textures: {t:?}");
        }
        if std::env::var_os("BONES_LIST").is_some() {
            let own: Vec<&String> = set
                .iter()
                .filter(|s| first.as_ref().is_none_or(|f| !f.contains(*s)))
                .collect();
            println!("  not in {}: {own:?}", models[0]);
        }
        if first.is_none() {
            first = Some(set);
        }
    }
}
