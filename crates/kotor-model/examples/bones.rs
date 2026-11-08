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
        // Where nodes sit at rest in the model, and the box of their mesh there: `BONES_AT=eyes,head_g`.
        if let Ok(at) = std::env::var("BONES_AT") {
            let want: Vec<String> = at.split(',').map(|s| s.to_ascii_lowercase()).collect();
            places(&m.root, [0.0; 3], [1.0, 0.0, 0.0, 0.0], &want);
        }
        if first.is_none() {
            first = Some(set);
        }
    }
}

fn turn(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [w, x, y, z] = q;
    let u = [x, y, z];
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let t = cross(u, v).map(|c| 2.0 * c);
    let c = cross(u, t);
    [0, 1, 2].map(|i| v[i] + w * t[i] + c[i])
}

fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

fn places(n: &kotor_model::Node, at: [f32; 3], q: [f32; 4], want: &[String]) {
    let p = turn(q, n.position);
    let at = [at[0] + p[0], at[1] + p[1], at[2] + p[2]];
    let q = mul(q, n.orientation);
    if want.iter().any(|w| *w == n.name.to_ascii_lowercase()) {
        let bounds = n
            .mesh
            .as_ref()
            .filter(|m| !m.positions.is_empty())
            .map(|m| {
                let mut lo = [f32::MAX; 3];
                let mut hi = [f32::MIN; 3];
                for v in &m.positions {
                    let v = turn(q, *v);
                    for i in 0..3 {
                        lo[i] = lo[i].min(at[i] + v[i]);
                        hi[i] = hi[i].max(at[i] + v[i]);
                    }
                }
                (lo, hi)
            });
        println!(
            "  {} at {at:.3?}, turned {q:.3?}, mesh box {bounds:.3?}",
            n.name
        );
    }
    for c in &n.children {
        places(c, at, q, want);
    }
}
