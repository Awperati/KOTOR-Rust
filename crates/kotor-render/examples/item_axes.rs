//! The extent of item models along each axis, for where a weapon sits: item_axes w_vbroswrd_001 w_stunbtn_001 ...
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_model::{parse_model, Node};

fn main() {
    let mut inst = Install::open(kotor_core::install_dir().expect("no install")).unwrap();
    for name in std::env::args().skip(1) {
        let (Ok(mdl), mdx) = (
            inst.read(&name, ResType::MDL),
            inst.read(&name, ResType::MDX).unwrap_or_default(),
        ) else {
            println!("{name}: none");
            continue;
        };
        let Ok(m) = parse_model(&mdl, &mdx) else {
            continue;
        };
        let mats = m.world_matrices(None);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        m.root.walk(&mut |n: &Node| {
            let (Some(mesh), Some((_, w))) =
                (n.mesh.as_ref(), mats.iter().find(|(x, _)| x == &n.name))
            else {
                return;
            };
            for p in &mesh.positions {
                for k in 0..3 {
                    let v = w[k][0] * p[0] + w[k][1] * p[1] + w[k][2] * p[2] + w[k][3];
                    lo[k] = lo[k].min(v);
                    hi[k] = hi[k].max(v);
                }
            }
        });
        println!(
            "{name}: x {:.2}..{:.2}  y {:.2}..{:.2}  z {:.2}..{:.2}",
            lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]
        );
    }
}
