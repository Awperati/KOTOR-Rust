//! Dialogue (DLG) as a typed model over GFF, plus a plain-text authoring format.
//!
//! Fields this model does not interpret (animations, camera, plot data, ...) are kept in `extra`
//! so a real DLG converts to a `Dialog` and back without loss.
//!
//! Text format (one block per node, blank lines ignored, `#` starts a comment line):
//!
//! ```text
//! start E0
//! start E1 if=k_condition
//! entry E0 speaker=Carth script=k_run vo=carth_01: Hello there.
//!   link R0
//!   link R1 if=k_cond
//! reply R0: Who are you?
//!   link E1
//! ```
//! Attributes are `key=value` with no spaces: `speaker`, `listener`, `script`, `vo`, `strref`, `comment`.
//! Text may contain `\n` and `\\` escapes. Labels are names you choose; they become list indices in order.

use crate::{Gff, LocString, Struct, Value};
use kotor_core::{Error, Result};
use std::collections::HashMap;
use std::fmt::Write as _;

type Fields = Vec<(String, Value)>;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Link {
    pub index: u32,
    /// Condition script resref; empty means always.
    pub active: String,
    pub is_child: Option<u8>,
    pub extra: Fields,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Node {
    pub speaker: Option<String>,
    pub listener: Option<String>,
    pub text: Option<LocString>,
    pub script: Option<String>,
    pub vo: Option<String>,
    pub comment: Option<String>,
    pub links: Vec<Link>,
    pub extra: Fields,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Dialog {
    pub starts: Vec<Link>,
    pub entries: Vec<Node>,
    pub replies: Vec<Node>,
    pub extra: Fields,
}

fn string_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) | Value::ResRef(s) => Some(s.clone()),
        _ => None,
    }
}

fn link_from(s: &Struct) -> Result<Link> {
    let mut l = Link::default();
    for (label, v) in &s.fields {
        match (label.as_str(), v) {
            ("Index", Value::Dword(i)) => l.index = *i,
            ("Active", v) if string_of(v).is_some() => l.active = string_of(v).unwrap_or_default(),
            ("IsChild", Value::Byte(b)) => l.is_child = Some(*b),
            _ => l.extra.push((label.clone(), v.clone())),
        }
    }
    Ok(l)
}

fn links_from(list: &[Struct]) -> Result<Vec<Link>> {
    list.iter().map(link_from).collect()
}

fn node_from(s: &Struct, link_label: &str) -> Result<Node> {
    let mut n = Node::default();
    for (label, v) in &s.fields {
        match (label.as_str(), v) {
            ("Speaker", v) if string_of(v).is_some() => n.speaker = string_of(v),
            ("Listener", v) if string_of(v).is_some() => n.listener = string_of(v),
            ("Script", v) if string_of(v).is_some() => n.script = string_of(v),
            ("VO_ResRef", v) if string_of(v).is_some() => n.vo = string_of(v),
            ("Comment", v) if string_of(v).is_some() => n.comment = string_of(v),
            ("Text", Value::LocString(t)) => n.text = Some(t.clone()),
            (l, Value::List(items)) if l == link_label => n.links = links_from(items)?,
            _ => n.extra.push((label.clone(), v.clone())),
        }
    }
    Ok(n)
}

fn link_to(l: &Link, position: usize) -> Struct {
    let mut fields: Fields = vec![
        ("Index".into(), Value::Dword(l.index)),
        ("Active".into(), Value::ResRef(l.active.clone())),
    ];
    if let Some(c) = l.is_child {
        fields.push(("IsChild".into(), Value::Byte(c)));
    }
    fields.extend(l.extra.iter().cloned());
    Struct {
        id: position as u32,
        fields,
    }
}

fn links_to(links: &[Link]) -> Value {
    Value::List(
        links
            .iter()
            .enumerate()
            .map(|(i, l)| link_to(l, i))
            .collect(),
    )
}

fn node_to(n: &Node, index: usize, link_label: &str) -> Struct {
    let mut f: Fields = Vec::new();
    if let Some(s) = &n.speaker {
        f.push(("Speaker".into(), Value::String(s.clone())));
    }
    if let Some(t) = &n.text {
        f.push(("Text".into(), Value::LocString(t.clone())));
    }
    if let Some(s) = &n.vo {
        f.push(("VO_ResRef".into(), Value::ResRef(s.clone())));
    }
    if let Some(s) = &n.script {
        f.push(("Script".into(), Value::ResRef(s.clone())));
    }
    if let Some(s) = &n.comment {
        f.push(("Comment".into(), Value::String(s.clone())));
    }
    if let Some(s) = &n.listener {
        f.push(("Listener".into(), Value::String(s.clone())));
    }
    f.extend(n.extra.iter().cloned());
    f.push((link_label.into(), links_to(&n.links)));
    Struct {
        id: index as u32,
        fields: f,
    }
}

impl Dialog {
    pub fn from_gff(g: &Gff) -> Result<Dialog> {
        if g.file_type != "DLG " {
            return Err(Error::format(format!("not a DLG: {}", g.file_type)));
        }
        let mut d = Dialog::default();
        for (label, v) in &g.root.fields {
            match (label.as_str(), v) {
                ("StartingList", Value::List(l)) => d.starts = links_from(l)?,
                ("EntryList", Value::List(l)) => {
                    d.entries = l
                        .iter()
                        .map(|s| node_from(s, "RepliesList"))
                        .collect::<Result<_>>()?
                }
                ("ReplyList", Value::List(l)) => {
                    d.replies = l
                        .iter()
                        .map(|s| node_from(s, "EntriesList"))
                        .collect::<Result<_>>()?
                }
                _ => d.extra.push((label.clone(), v.clone())),
            }
        }
        Ok(d)
    }

    /// Link targets must exist: starts and reply links point at entries, entry links at replies.
    pub fn check(&self) -> Result<()> {
        let bad = |what: &str, i: u32| Error::format(format!("{what} points at missing node {i}"));
        for l in &self.starts {
            if l.index as usize >= self.entries.len() {
                return Err(bad("start", l.index));
            }
        }
        for n in &self.entries {
            if let Some(l) = n
                .links
                .iter()
                .find(|l| l.index as usize >= self.replies.len())
            {
                return Err(bad("entry link", l.index));
            }
        }
        for n in &self.replies {
            if let Some(l) = n
                .links
                .iter()
                .find(|l| l.index as usize >= self.entries.len())
            {
                return Err(bad("reply link", l.index));
            }
        }
        Ok(())
    }

    pub fn to_gff(&self) -> Gff {
        let mut f: Fields = self.extra.clone();
        f.push(("StartingList".into(), links_to(&self.starts)));
        f.push((
            "EntryList".into(),
            Value::List(
                self.entries
                    .iter()
                    .enumerate()
                    .map(|(i, n)| node_to(n, i, "RepliesList"))
                    .collect(),
            ),
        ));
        f.push((
            "ReplyList".into(),
            Value::List(
                self.replies
                    .iter()
                    .enumerate()
                    .map(|(i, n)| node_to(n, i, "EntriesList"))
                    .collect(),
            ),
        ));
        Gff {
            file_type: "DLG ".into(),
            root: Struct {
                id: u32::MAX,
                fields: f,
            },
        }
    }

    /// Typical root fields of a hand-authored conversation (values copied from what the stock files use).
    pub fn defaults() -> Dialog {
        let e = |l: &str, v: Value| (l.to_string(), v);
        Dialog {
            extra: vec![
                e("DelayEntry", Value::Dword(0)),
                e("DelayReply", Value::Dword(0)),
                e("NumWords", Value::Dword(0)),
                e("EndConversation", Value::ResRef(String::new())),
                e("EndConverAbort", Value::ResRef(String::new())),
                e("Skippable", Value::Byte(1)),
                e("StuntList", Value::List(vec![])),
                e("CameraModel", Value::ResRef(String::new())),
                e("VO_ID", Value::String(String::new())),
                e("ConversationType", Value::Int(0)),
                e("ComputerType", Value::Byte(0)),
                e("OldHitCheck", Value::Byte(0)),
                e("AmbientTrack", Value::ResRef(String::new())),
                e("UnequipItems", Value::Byte(0)),
                e("AnimatedCut", Value::Byte(0)),
                e("UnequipHItem", Value::Byte(0)),
            ],
            ..Dialog::default()
        }
    }
}

impl Node {
    /// A node carrying the per-node fields every stock line has.
    pub fn authored(text: LocString) -> Node {
        let e = |l: &str, v: Value| (l.to_string(), v);
        Node {
            text: Some(text),
            script: Some(String::new()),
            vo: Some(String::new()),
            comment: Some(String::new()),
            listener: Some(String::new()),
            extra: vec![
                e("AnimList", Value::List(vec![])),
                e("Delay", Value::Dword(u32::MAX)),
                e("Sound", Value::ResRef(String::new())),
                e("Quest", Value::String(String::new())),
                e("PlotIndex", Value::Int(-1)),
                e("PlotXPPercentage", Value::Float(1.0)),
                e("WaitFlags", Value::Dword(0)),
                e("CameraAngle", Value::Dword(0)),
                e("FadeType", Value::Byte(0)),
            ],
            ..Node::default()
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "")
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn attr_ok(v: &str) -> bool {
    !v.is_empty() && !v.chars().any(|c| c.is_whitespace() || c == ':')
}

impl Dialog {
    /// Render in the authoring format. Only the fields the format covers are written; labels are `E<n>` / `R<n>`.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let link = |l: &Link, kind: char| {
            let cond = if l.active.is_empty() {
                String::new()
            } else {
                format!(" if={}", l.active)
            };
            format!("{kind}{}{cond}", l.index)
        };
        for l in &self.starts {
            let _ = writeln!(s, "start {}", link(l, 'E'));
        }
        let mut node = |n: &Node, i: usize, kw: &str, label: char, kind: char| {
            let _ = write!(s, "\n{kw} {label}{i}");
            for (k, v) in [
                ("speaker", &n.speaker),
                ("listener", &n.listener),
                ("script", &n.script),
                ("vo", &n.vo),
                ("comment", &n.comment),
            ] {
                if let Some(v) = v.as_ref().filter(|v| attr_ok(v)) {
                    let _ = write!(s, " {k}={v}");
                }
            }
            let t = n.text.as_ref();
            if let Some(strref) = t.map(|t| t.strref).filter(|&r| r >= 0) {
                let _ = write!(s, " strref={strref}");
            }
            let text = t
                .and_then(|t| t.strings.first())
                .map(|(_, x)| x.as_str())
                .unwrap_or("");
            let _ = writeln!(s, ": {}", escape(text));
            for l in &n.links {
                let _ = writeln!(s, "  link {}", link(l, kind));
            }
        };
        for (i, n) in self.entries.iter().enumerate() {
            node(n, i, "entry", 'E', 'R');
        }
        for (i, n) in self.replies.iter().enumerate() {
            node(n, i, "reply", 'R', 'E');
        }
        s
    }

    /// Parse the authoring format into a ready-to-write dialogue with stock default fields.
    pub fn from_text(src: &str) -> Result<Dialog> {
        struct Pending {
            reply: bool,
            label: String,
            node: Node,
            links: Vec<(String, String)>,
        }
        let mut d = Dialog::defaults();
        let mut starts: Vec<(String, String)> = Vec::new();
        let mut nodes: Vec<Pending> = Vec::new();
        let err = |n: usize, m: &str| Error::format(format!("dialogue text line {n}: {m}"));

        // `E0 if=cond` style target with optional condition.
        let target = |rest: &str, n: usize| -> Result<(String, String)> {
            let mut p = rest.split_whitespace();
            let label = p
                .next()
                .ok_or_else(|| err(n, "missing target"))?
                .to_string();
            let mut cond = String::new();
            for extra in p {
                cond = extra
                    .strip_prefix("if=")
                    .ok_or_else(|| err(n, "expected if=<script>"))?
                    .to_string();
            }
            Ok((label, cond))
        };

        for (i, raw) in src.lines().enumerate() {
            let n = i + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("start ") {
                starts.push(target(rest, n)?);
            } else if let Some(rest) = line.strip_prefix("link ") {
                let (l, c) = target(rest, n)?;
                nodes
                    .last_mut()
                    .ok_or_else(|| err(n, "link before any node"))?
                    .links
                    .push((l, c));
            } else if line.starts_with("entry ") || line.starts_with("reply ") {
                let reply = line.starts_with("reply ");
                let (head, text) = line[6..]
                    .split_once(':')
                    .ok_or_else(|| err(n, "missing ':' before the line text"))?;
                let mut parts = head.split_whitespace();
                let label = parts
                    .next()
                    .ok_or_else(|| err(n, "missing label"))?
                    .to_string();
                let mut node = Node::authored(LocString {
                    strref: -1,
                    strings: vec![],
                });
                if !reply {
                    node.speaker = Some(String::new());
                }
                let mut strref = -1;
                for a in parts {
                    let (k, v) = a
                        .split_once('=')
                        .ok_or_else(|| err(n, "attribute must be key=value"))?;
                    match k {
                        "speaker" if !reply => node.speaker = Some(v.to_string()),
                        "listener" => node.listener = Some(v.to_string()),
                        "script" => node.script = Some(v.to_string()),
                        "vo" => node.vo = Some(v.to_string()),
                        "comment" => node.comment = Some(v.to_string()),
                        "strref" => {
                            strref = v.parse().map_err(|_| err(n, "strref must be a number"))?
                        }
                        _ => return Err(err(n, &format!("unknown attribute '{k}'"))),
                    }
                }
                let text = unescape(text.trim_start());
                node.text = Some(LocString {
                    strref,
                    strings: if text.is_empty() {
                        vec![]
                    } else {
                        vec![(0, text)]
                    },
                });
                nodes.push(Pending {
                    reply,
                    label,
                    node,
                    links: vec![],
                });
            } else {
                return Err(err(n, "expected start, entry, reply or link"));
            }
        }

        let mut entry_ids: HashMap<String, u32> = HashMap::new();
        let mut reply_ids: HashMap<String, u32> = HashMap::new();
        for p in &nodes {
            let (map, len) = if p.reply {
                (&mut reply_ids, d.replies.len())
            } else {
                (&mut entry_ids, d.entries.len())
            };
            if map.insert(p.label.clone(), len as u32).is_some() {
                return Err(Error::format(format!("duplicate label {}", p.label)));
            }
            if p.reply {
                d.replies.push(Node::default())
            } else {
                d.entries.push(Node::default())
            }
        }
        let mk = |label: &str, cond: &str, ids: &HashMap<String, u32>| -> Result<Link> {
            let index = *ids
                .get(label)
                .ok_or_else(|| Error::format(format!("unknown label {label}")))?;
            Ok(Link {
                index,
                active: cond.to_string(),
                is_child: Some(0),
                extra: vec![],
            })
        };
        for (label, cond) in &starts {
            d.starts.push(Link {
                is_child: None,
                ..mk(label, cond, &entry_ids)?
            });
        }
        let (mut ei, mut ri) = (0, 0);
        let mut words = 0;
        for p in nodes {
            let mut node = p.node;
            let targets = if p.reply { &entry_ids } else { &reply_ids };
            for (l, c) in &p.links {
                node.links.push(mk(l, c, targets)?);
            }
            words += node
                .text
                .as_ref()
                .and_then(|t| t.strings.first())
                .map(|(_, t)| t.split_whitespace().count())
                .unwrap_or(0);
            if p.reply {
                d.replies[ri] = node;
                ri += 1;
            } else {
                d.entries[ei] = node;
                ei += 1;
            }
        }
        if let Some(f) = d.extra.iter_mut().find(|(l, _)| l == "NumWords") {
            f.1 = Value::Dword(words as u32);
        }
        d.check()?;
        Ok(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# a small conversation
start E0
start E1 if=k_cond

entry E0 speaker=Carth script=k_run: Hello there.\\nSecond line.
  link R0
  link R1 if=k_x
entry E1 speaker=Carth: Back again?

reply R0: Who are you?
  link E1
reply R1 strref=1234:
";

    #[test]
    fn text_round_trip_and_gff() {
        let d = Dialog::from_text(SAMPLE).unwrap();
        assert_eq!(d.entries.len(), 2);
        assert_eq!(d.replies.len(), 2);
        assert_eq!(d.starts.len(), 2);
        assert_eq!(d.starts[1].active, "k_cond");
        assert_eq!(d.entries[0].links[1].active, "k_x");
        let text0 = &d.entries[0].text.as_ref().unwrap().strings[0].1;
        assert_eq!(text0, "Hello there.\nSecond line.");
        assert_eq!(d.replies[1].text.as_ref().unwrap().strref, 1234);

        let g = d.to_gff();
        let back = Dialog::from_gff(&Gff::parse(&g.to_bytes().unwrap()).unwrap()).unwrap();
        assert_eq!(back, d);
        assert_eq!(
            Dialog::from_text(&d.to_text()).unwrap().to_text(),
            d.to_text()
        );
    }

    #[test]
    fn errors_are_specific() {
        assert!(Dialog::from_text("start E9\n").is_err());
        assert!(Dialog::from_text("entry E0 bogus=1: hi\n").is_err());
        assert!(Dialog::from_text("link R0\n").is_err());
        assert!(Dialog::from_text("entry E0: a\nentry E0: b\n").is_err());
        assert!(Dialog::from_text("entry E0: a\n  link R5\n").is_err());
    }
}
