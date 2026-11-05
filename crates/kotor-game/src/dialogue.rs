//! The dialogue runner: walks a DLG graph, running condition and action scripts, and records what is said.

use crate::world::{ObjId, World};
use kotor_core::ResType;
use kotor_gff::dlg::{Dialog, Node};
use kotor_gff::{Gff, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// Spoken by a character in the conversation (a DLG entry).
    Npc,
    /// The player's chosen reply (a DLG reply).
    Player,
}

/// One line of a played conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub speaker: String,
    pub text: String,
    pub kind: LineKind,
    /// Index of the entry or reply in its DLG list.
    pub node: usize,
    /// The dialogue the line came from.
    pub dialogue: String,
}

/// Picks the player's reply when more than one is available.
pub trait Chooser {
    /// `options` are the texts of the available replies; return the index of the one chosen.
    fn choose(&mut self, options: &[String]) -> usize;
}

/// Always takes the first available reply.
pub struct FirstReply;

impl Chooser for FirstReply {
    fn choose(&mut self, _options: &[String]) -> usize {
        0
    }
}

/// Stop a conversation that loops forever.
const MAX_NODES: usize = 400;

impl World {
    fn node_text(&self, n: &Node) -> String {
        n.text.as_ref().map(|t| self.text(t)).unwrap_or_default()
    }

    fn speaker_name(&self, n: &Node, owner: ObjId) -> String {
        let tag = n.speaker.as_deref().unwrap_or("");
        if tag.is_empty() {
            return self.describe_short(owner);
        }
        match self.by_tag(tag).first().and_then(|id| self.object(*id)) {
            Some(o) if !o.name.is_empty() => o.name.clone(),
            _ => tag.to_string(),
        }
    }

    fn describe_short(&self, id: ObjId) -> String {
        match self.object(id) {
            Some(o) if !o.name.is_empty() => o.name.clone(),
            Some(o) => o.tag.clone(),
            None => format!("#{id}"),
        }
    }

    fn run_node_script(&mut self, n: &Node, owner: ObjId) {
        if let Some(s) = n.script.as_deref().filter(|s| !s.is_empty()) {
            let _ = self.run_script(s, owner);
        }
        // Journal entries ride on the node as `Quest` and `PlotIndex`.
        let quest = n.extra.iter().find_map(|(l, v)| match (l.as_str(), v) {
            ("Quest", Value::String(q)) if !q.is_empty() => Some(q.clone()),
            _ => None,
        });
        if let Some(q) = quest {
            let idx = n.extra.iter().find_map(|(l, v)| match (l.as_str(), v) {
                ("PlotIndex", Value::Int(i)) => Some(*i),
                _ => None,
            });
            self.events
                .push(format!("journal: quest {q} entry {idx:?}"));
        }
    }

    /// Play a conversation between `owner` (who speaks the entries) and `listener` (the player), choosing
    /// player replies with `chooser`. Returns the number of lines recorded in `transcript`.
    ///
    /// The first start node whose condition passes begins it. Each entry runs its script and speaks; the
    /// available replies are those whose link condition passes. Replies with no text are "continue" links
    /// and are followed without being said. When no entry follows, the dialogue's `EndConversation` script runs.
    pub fn converse(
        &mut self,
        owner: ObjId,
        listener: ObjId,
        dialogue: &str,
        chooser: &mut dyn Chooser,
    ) -> Result<usize, String> {
        let _ = listener;
        let name = if dialogue.is_empty() {
            self.object(owner)
                .map(|o| o.conversation.clone())
                .unwrap_or_default()
        } else {
            dialogue.to_string()
        };
        if name.is_empty() {
            return Err(format!("{} has no conversation", self.describe(owner)));
        }
        let bytes = self
            .inst
            .read(&name, ResType::DLG)
            .map_err(|e| format!("dialogue {name}: {e}"))?;
        let gff = Gff::parse(&bytes).map_err(|e| format!("dialogue {name}: {e}"))?;
        let d = Dialog::from_gff(&gff).map_err(|e| format!("dialogue {name}: {e}"))?;
        d.check().map_err(|e| format!("dialogue {name}: {e}"))?;
        let before = self.transcript.len();
        let mut steps: Vec<crate::cutscene::Step> = Vec::new();
        self.in_conversation = true;
        self.events.push(format!(
            "conversation {name} begins with {}",
            self.describe(owner)
        ));

        // First start node whose condition passes.
        let mut next = None;
        for s in &d.starts {
            if self.check(&s.active, owner) {
                next = Some(s.index as usize);
                break;
            }
        }
        let mut visited = 0;
        while let Some(ei) = next.take() {
            visited += 1;
            if visited > MAX_NODES {
                self.errors
                    .push(format!("dialogue {name}: stopped after {MAX_NODES} nodes"));
                break;
            }
            let entry = &d.entries[ei];
            let ev0 = self.events.len();
            self.run_node_script(entry, owner);
            let text = self.node_text(entry);
            steps.push(crate::cutscene::step_of(
                entry,
                LineKind::Npc,
                ei,
                self.speaker_name(entry, owner),
                text.clone(),
                &self.events[ev0..],
            ));
            if !text.is_empty() {
                let speaker = self.speaker_name(entry, owner);
                self.transcript.push(Line {
                    speaker,
                    text,
                    kind: LineKind::Npc,
                    node: ei,
                    dialogue: name.clone(),
                });
            }
            // Available replies.
            let mut options: Vec<(usize, String)> = Vec::new();
            for l in &entry.links {
                if self.check(&l.active, owner) {
                    let r = l.index as usize;
                    options.push((r, self.node_text(&d.replies[r])));
                }
            }
            if options.is_empty() {
                break;
            }
            // Continue links (no text) are followed automatically; otherwise the player picks among the texts.
            let pick = if options.iter().all(|(_, t)| t.is_empty()) {
                0
            } else {
                let shown: Vec<String> = options
                    .iter()
                    .filter(|(_, t)| !t.is_empty())
                    .map(|(_, t)| t.clone())
                    .collect();
                let k = chooser.choose(&shown).min(shown.len().saturating_sub(1));
                options
                    .iter()
                    .position(|(_, t)| *t == shown[k])
                    .unwrap_or(0)
            };
            let (ri, rtext) = options[pick].clone();
            let reply = &d.replies[ri];
            if !rtext.is_empty() {
                let who = self.describe_short(self.pc);
                self.transcript.push(Line {
                    speaker: who,
                    text: rtext.clone(),
                    kind: LineKind::Player,
                    node: ri,
                    dialogue: name.clone(),
                });
            }
            let ev1 = self.events.len();
            self.run_node_script(reply, owner);
            steps.push(crate::cutscene::step_of(
                reply,
                LineKind::Player,
                ri,
                self.describe_short(self.pc),
                rtext.clone(),
                &self.events[ev1..],
            ));
            for l in &reply.links {
                if self.check(&l.active, owner) {
                    next = Some(l.index as usize);
                    break;
                }
            }
        }
        // Scripts that end a conversation.
        let end_script = d.extra.iter().find_map(|(l, v)| match (l.as_str(), v) {
            ("EndConversation", Value::ResRef(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        });
        if let Some(s) = end_script {
            let _ = self.run_script(&s, owner);
        }
        self.events.push(format!(
            "conversation {name} ends after {} lines, {visited} nodes",
            self.transcript.len() - before
        ));
        self.in_conversation = false;
        let stunts = d
            .extra
            .iter()
            .find_map(|(l, v)| match (l.as_str(), v) {
                ("StuntList", Value::List(items)) => Some(
                    items
                        .iter()
                        .filter_map(|s| {
                            Some((
                                s.str("Participant")?.to_string(),
                                s.str("StuntModel")?.to_string(),
                            ))
                        })
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .unwrap_or_default();
        let camera_model = d
            .extra
            .iter()
            .find_map(|(l, v)| match (l.as_str(), v) {
                ("CameraModel", Value::ResRef(s)) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();
        self.conversation = Some(crate::cutscene::Conversation {
            name: name.clone(),
            camera_model,
            stunts,
            steps,
        });
        Ok(self.transcript.len() - before)
    }
}
