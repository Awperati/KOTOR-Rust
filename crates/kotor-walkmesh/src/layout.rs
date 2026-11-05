use kotor_core::{latin1, Error, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct Room {
    /// Room model name (also the name of its walkmesh).
    pub model: String,
    pub position: [f32; 3],
}

/// An area layout: where each room model sits. Other sections (door hooks,
/// tracks, obstacles) are kept as raw line counts only for now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub rooms: Vec<Room>,
    pub door_hooks: usize,
    pub tracks: usize,
    pub obstacles: usize,
}

fn floats(parts: &[&str]) -> Result<[f32; 3]> {
    let f = |i: usize| -> Result<f32> {
        parts
            .get(i)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| Error::format("bad number in layout line"))
    };
    Ok([f(0)?, f(1)?, f(2)?])
}

pub fn parse_lyt(data: &[u8]) -> Result<Layout> {
    let text = latin1(data);
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let mut lay = Layout::default();
    while let Some(line) = lines.next() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let count = |p: &[&str]| -> Result<usize> {
            p.get(1)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| Error::format("bad count in layout"))
        };
        match parts[0].to_ascii_lowercase().as_str() {
            "roomcount" => {
                for _ in 0..count(&parts)? {
                    let l = lines
                        .next()
                        .ok_or_else(|| Error::format("layout ends in rooms"))?;
                    let p: Vec<&str> = l.split_whitespace().collect();
                    let model = p.first().ok_or_else(|| Error::format("empty room line"))?;
                    lay.rooms.push(Room {
                        model: model.to_ascii_lowercase(),
                        position: floats(&p[1..])?,
                    });
                }
            }
            "doorhookcount" => {
                lay.door_hooks = count(&parts)?;
                for _ in 0..lay.door_hooks {
                    lines.next();
                }
            }
            "trackcount" => {
                lay.tracks = count(&parts)?;
                for _ in 0..lay.tracks {
                    lines.next();
                }
            }
            "obstaclecount" => {
                lay.obstacles = count(&parts)?;
                for _ in 0..lay.obstacles {
                    lines.next();
                }
            }
            _ => {}
        }
    }
    if lay.rooms.is_empty() {
        return Err(Error::format("layout has no rooms"));
    }
    Ok(lay)
}

/// One room and the rooms visible from it.
#[derive(Clone, Debug, PartialEq)]
pub struct VisEntry {
    pub room: String,
    pub visible: Vec<String>,
}

pub fn parse_vis(data: &[u8]) -> Result<Vec<VisEntry>> {
    let text = latin1(data);
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let mut out = Vec::new();
    while let Some(head) = lines.next() {
        let p: Vec<&str> = head.split_whitespace().collect();
        let n: usize = p
            .get(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| Error::format(format!("bad vis header '{head}'")))?;
        let mut visible = Vec::with_capacity(n);
        for _ in 0..n {
            let l = lines
                .next()
                .ok_or_else(|| Error::format("vis ends early"))?;
            visible.push(l.to_ascii_lowercase());
        }
        out.push(VisEntry {
            room: p[0].to_ascii_lowercase(),
            visible,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lyt_rooms() {
        let t = b"beginlayout\n  roomcount 2\n    M01aa_01a 0.0 10.5 0.0\n    m01aa_02a -3 0 0\n  doorhookcount 0\ndonelayout\n";
        let l = parse_lyt(t).unwrap();
        assert_eq!(l.rooms.len(), 2);
        assert_eq!(l.rooms[0].model, "m01aa_01a");
        assert_eq!(l.rooms[0].position, [0.0, 10.5, 0.0]);
    }

    #[test]
    fn vis_entries() {
        let v = parse_vis(b"a 1\n b\nb 1\n a\n").unwrap();
        assert_eq!(v[1].visible, ["a"]);
        assert!(parse_vis(b"a 2\n b\n").is_err());
    }
}
