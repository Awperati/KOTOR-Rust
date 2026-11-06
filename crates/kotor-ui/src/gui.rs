//! GUI files (`.gui`, a GFF whose root holds a panel and a `CONTROLS` list), parsed into plain structs.
//!
//! Coordinates are in the 640x480 layout space the files are written for; a control's extent is relative to its
//! panel. Control types seen: 2 panel, 4 label, 5 button, 6 toggle, 8 slider, 9 scrollbar, 10 progress bar,
//! 11 list box. Text alignment is read as bits: horizontal 1 left, 2 centre, 4 right; vertical 8 top, 16 middle,
//! 32 bottom. **Read from the values, not checked against the game:** the files use 9, 10, 17, 18, 20 and 34, the
//! buttons are 18, the credits figure is 20 and the notice at the foot of the main menu is 34.

use kotor_core::{Error, Result};
use kotor_gff::{Gff, Struct, Value};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Extent {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Border {
    pub corner: String,
    pub edge: String,
    pub fill: String,
    pub dimension: i32,
    pub inner_offset: i32,
    pub color: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct GuiText {
    pub font: String,
    pub text: String,
    pub strref: Option<u32>,
    pub color: [f32; 3],
    pub alignment: i32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Control {
    pub kind: i32,
    pub tag: String,
    pub extent: Extent,
    pub border: Border,
    /// What a button shows under the cursor.
    pub hilight: Option<Border>,
    /// What a checkbox shows when it is ticked.
    pub selected: Option<Border>,
    /// The picture of a slider's handle.
    pub thumb: Option<String>,
    pub text: Option<GuiText>,
    /// Progress bars: the fill drawn for the filled part, and the value range.
    pub progress_fill: Option<Border>,
    pub cur_value: i32,
    pub max_value: i32,
    pub from_left: bool,
    /// List boxes: the item template and the padding between items.
    pub proto_item: Option<Box<Control>>,
    pub padding: i32,
    pub children: Vec<Control>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Gui {
    /// The panel itself: position on screen, size, border.
    pub panel: Control,
}

fn extent(s: &Struct) -> Extent {
    let g = |l: &str| s.int(l).unwrap_or(0) as i32;
    Extent {
        left: g("LEFT"),
        top: g("TOP"),
        width: g("WIDTH"),
        height: g("HEIGHT"),
    }
}

fn color(s: &Struct) -> [f32; 3] {
    match s.get("COLOR") {
        Some(Value::Vector(v)) => *v,
        _ => [1.0; 3],
    }
}

fn border(s: &Struct) -> Border {
    let r = |l: &str| s.str(l).unwrap_or("").to_string();
    Border {
        corner: r("CORNER"),
        edge: r("EDGE"),
        fill: r("FILL"),
        dimension: s.int("DIMENSION").unwrap_or(0) as i32,
        inner_offset: s.int("INNEROFFSET").unwrap_or(0) as i32,
        color: color(s),
    }
}

fn control(s: &Struct) -> Control {
    let text = s.strukt("TEXT").map(|t| GuiText {
        font: t.str("FONT").unwrap_or("").to_string(),
        text: t.str("TEXT").unwrap_or("").to_string(),
        strref: t
            .int("STRREF")
            .filter(|&v| v != 0xFFFF_FFFF)
            .map(|v| v as u32),
        color: color(t),
        alignment: t.int("ALIGNMENT").unwrap_or(1) as i32,
    });
    let mut children: Vec<Control> = s
        .list("CONTROLS")
        .map(|l| l.iter().map(control).collect())
        .unwrap_or_default();
    if let Some(sb) = s.strukt("SCROLLBAR") {
        children.push(control(sb));
    }
    Control {
        kind: s.int("CONTROLTYPE").unwrap_or(0) as i32,
        tag: s.str("TAG").unwrap_or("").to_string(),
        extent: s.strukt("EXTENT").map(extent).unwrap_or_default(),
        border: s.strukt("BORDER").map(border).unwrap_or_default(),
        hilight: s.strukt("HILIGHT").map(border),
        selected: s.strukt("SELECTED").map(border),
        thumb: s
            .strukt("THUMB")
            .and_then(|t| t.str("IMAGE").map(str::to_string)),
        text,
        progress_fill: s.strukt("PROGRESS").map(border),
        cur_value: s.int("CURVALUE").unwrap_or(0) as i32,
        max_value: s.int("MAXVALUE").unwrap_or(0) as i32,
        from_left: s.int("STARTFROMLEFT").unwrap_or(1) != 0,
        proto_item: s.strukt("PROTOITEM").map(|p| Box::new(control(p))),
        padding: s.int("PADDING").unwrap_or(0) as i32,
        children,
    }
}

impl Gui {
    pub fn parse(data: &[u8]) -> Result<Gui> {
        let g = Gff::parse(data)?;
        if !g.file_type.starts_with("GUI") {
            return Err(Error::format(format!("not a GUI file: {:?}", g.file_type)));
        }
        Ok(Gui {
            panel: control(&g.root),
        })
    }

    /// The control with this tag, searching children too.
    pub fn find(&self, tag: &str) -> Option<&Control> {
        fn go<'a>(c: &'a Control, tag: &str) -> Option<&'a Control> {
            if c.tag == tag {
                return Some(c);
            }
            c.children.iter().find_map(|k| go(k, tag))
        }
        go(&self.panel, tag)
    }
}
