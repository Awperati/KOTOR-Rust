//! Print a GUI file's controls: kind, tag, extent, textures, font and text.
//!
//!   cargo run --offline -p kotor-ui --example gui_dump -- mainmenu16x12 optionsingame
use kotor_archive::Install;
use kotor_ui::{Control, Ui};

fn show(c: &Control, depth: usize, ui: &mut Ui) {
    let e = &c.extent;
    let text = c.text.as_ref().map(|t| {
        let s = match t.strref {
            Some(r) if t.text.is_empty() => format!("#{r} {:?}", ui.strref(r)),
            _ => format!("{:?}", t.text),
        };
        format!("{} a{} {s}", t.font, t.alignment)
    });
    println!(
        "{:depth$}{} {:?} [{} {} {} {}] fill={:?} edge={:?} corner={:?} dim={} {}",
        "",
        c.kind,
        c.tag,
        e.left,
        e.top,
        e.width,
        e.height,
        c.border.fill,
        c.border.edge,
        c.border.corner,
        c.border.dimension,
        text.unwrap_or_default(),
        depth = depth * 2,
    );
    if let Some(p) = &c.proto_item {
        print!("{:depth$}proto ", "", depth = depth * 2 + 2);
        show(p, 0, ui);
    }
    for k in &c.children {
        show(k, depth + 1, ui);
    }
}

fn main() {
    let dir = kotor_core::install_dir().expect("no install");
    let inst: &'static mut Install = Box::leak(Box::new(Install::open(dir).unwrap()));
    let mut ui = Ui::new(inst);
    for name in std::env::args().skip(1) {
        println!("== {name}");
        match ui.gui(&name) {
            Some(g) => show(&g.panel, 0, &mut ui),
            None => println!("no such GUI"),
        }
    }
}
