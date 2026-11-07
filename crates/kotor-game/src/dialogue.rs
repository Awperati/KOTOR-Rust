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

    /// Who says the line on now, and to whom: the node's Speaker, or else the conversation's owner, and its Listener, or else
    /// the player. `PLAYER` is the player either way. With no line held, the owner and the player.
    ///
    /// **Assumed:** that an empty Listener means the player. The data has it so on lines plainly said to the player, and the
    /// original's own rule has not been read.
    pub fn dialogue_pair(&self) -> Option<(ObjId, ObjId)> {
        let run = self.conv.as_ref()?;
        let Some(held) = run.held.as_ref() else {
            return Some((run.owner, self.pc));
        };
        let who = |tag: &str| -> Option<ObjId> {
            // The player's own tag is empty: an empty field is not the player by that road.
            if tag.is_empty() {
                return None;
            }
            if tag.eq_ignore_ascii_case("player") {
                return Some(self.pc);
            }
            self.by_tag(tag).first().copied()
        };
        let speaker = who(&held.speaker_tag).unwrap_or(run.owner);
        let spoken_to = who(&held.listener_tag).unwrap_or(self.pc);
        // The player, spoken for: to whoever the conversation is with.
        let listener = if spoken_to == speaker {
            run.owner
        } else {
            spoken_to
        };
        Some((speaker, listener))
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
            if !self.run_story_script(s) {
                let _ = self.run_script(s, owner);
            }
        }
        // Journal entries ride on the node as `Quest` and `QuestEntry`. `PlotIndex` is -1 on nearly all of them.
        let quest = n.extra.iter().find_map(|(l, v)| match (l.as_str(), v) {
            ("Quest", Value::String(q)) if !q.is_empty() => Some(q.clone()),
            _ => None,
        });
        if let Some(q) = quest {
            let idx = n.extra.iter().find_map(|(l, v)| match (l.as_str(), v) {
                ("QuestEntry", Value::Dword(i)) => Some(*i as i32),
                _ => None,
            });
            self.events
                .push(format!("journal: quest {q} entry {idx:?}"));
            if let Some(state) = idx.filter(|i| *i >= 0) {
                if self.journal.get(&q).is_none_or(|old| *old < state) {
                    self.set_journal(&q, state);
                }
            }
        }
    }

    /// Play a conversation between `owner` (who speaks the entries) and `listener` (the player), choosing
    /// player replies with `chooser`. Returns the number of lines recorded in `transcript`.
    ///
    /// The first start node whose condition passes begins it. Each entry runs its script and speaks; the
    /// available replies are those whose link condition passes. Replies with no text are "continue" links
    /// and are followed without being said. When no entry follows, the dialogue's `EndConversation` script runs.
    ///
    /// With `interactive_dialogue` set the chooser is ignored: the conversation runs up to the first line the
    /// player has to answer or acknowledge and waits there (see `dialogue_view` and `dialogue_choose`).
    pub fn converse(
        &mut self,
        owner: ObjId,
        listener: ObjId,
        dialogue: &str,
        chooser: &mut dyn Chooser,
    ) -> Result<usize, String> {
        let _ = listener;
        if self.interactive_dialogue && (self.conv.is_some() || self.in_conversation) {
            return Ok(0);
        }
        let before = self.transcript.len();
        let mut run = self.conv_begin(owner, dialogue)?;
        if self.interactive_dialogue {
            if self.conv_drive(&mut run, None, None) {
                self.conv_finish(run);
            } else {
                self.conv = Some(run);
            }
        } else {
            self.conv_drive(&mut run, None, Some(chooser));
            self.conv_finish(run);
        }
        Ok(self.transcript.len() - before)
    }

    /// What the player sees of a held conversation: speaker, the line spoken, its voice-over resref, and the
    /// numbered replies (empty when the line only needs acknowledging).
    pub fn dialogue_view(&self) -> Option<(String, String, String, Vec<String>)> {
        let h = self.conv.as_ref()?.held.as_ref()?;
        Some((
            h.speaker.clone(),
            h.text.clone(),
            h.vo.clone(),
            h.shown.clone(),
        ))
    }

    /// The held node as a one-step conversation, for the cutscene sequencer.
    pub fn dialogue_conversation(&self) -> Option<crate::cutscene::Conversation> {
        let run = self.conv.as_ref()?;
        Some(crate::cutscene::Conversation {
            name: run.name.clone(),
            camera_model: run.camera_model.clone(),
            stunts: run.stunts.clone(),
            steps: vec![run.held.as_ref()?.step.clone()],
        })
    }

    /// Goes up with every node of a held conversation.
    pub fn dialogue_serial(&self) -> Option<usize> {
        self.conv.as_ref().map(|c| c.visited)
    }

    /// Whether an `ActionPauseConversation` is in force or waiting in someone's queue.
    pub fn conversation_paused(&self) -> bool {
        self.conv_paused
            || self.objects.values().any(|o| {
                o.actions
                    .iter()
                    .any(|a| matches!(a, crate::world::Action::Recorded(r) if r == "ActionPauseConversation"))
            })
    }

    /// A pause nobody will lift, because whoever was to do it has died or lost its queue, must not end the game.
    ///
    /// **Assumption:** no cutscene means to wait this long; `tar03_cutdlg` is the case that runs into it.
    fn end_stale_pause(&mut self) {
        if self.conv.is_none() || !self.conversation_paused() {
            self.conv_paused_at = None;
            return;
        }
        let since = *self.conv_paused_at.get_or_insert(self.time);
        if self.time - since < crate::world::PAUSE_LIMIT_SECONDS {
            return;
        }
        self.conv_paused_at = None;
        self.conv_paused = false;
        for o in self.objects.values_mut() {
            o.actions.retain(|a| {
                !matches!(a, crate::world::Action::Recorded(r) if r == "ActionPauseConversation")
            });
        }
        let name = self.conv.as_ref().map_or(String::new(), |c| c.name.clone());
        self.errors
            .push(format!("dialogue {name}: a pause ran out"));
    }

    /// Pick a conversation up again once its pause is over.
    pub(crate) fn resume_conversation(&mut self) {
        self.end_stale_pause();
        let waiting = self.conv.as_ref().is_some_and(|c| c.waiting);
        if !waiting || self.conversation_paused() {
            return;
        }
        let Some(mut run) = self.conv.take() else {
            return;
        };
        if self.conv_drive(&mut run, None, None) {
            self.conv_finish(run);
        } else {
            self.conv = Some(run);
        }
    }

    /// The scripts reply `k` runs as far as the next choice, and whether there is a next choice. Conditions on the way are
    /// not weighed.
    fn what_reply_leads_to(&self, k: usize) -> Option<(Vec<String>, bool)> {
        let run = self.conv.as_ref()?;
        let h = run.held.as_ref()?;
        let first = h.shown.get(k).and_then(|t| {
            h.options
                .iter()
                .find_map(|(r, text)| (text == t).then_some(*r))
        })?;
        let d = &run.dialog;
        let mut scripts: Vec<String> = Vec::new();
        let mut choice_next = false;
        let mut seen = std::collections::BTreeSet::new();
        let mut todo = vec![first];
        while let Some(r) = todo.pop() {
            if !seen.insert(r) {
                continue;
            }
            let reply = &d.replies[r];
            scripts.extend(reply.script.clone());
            for l in &reply.links {
                let entry = &d.entries[l.index as usize];
                scripts.extend(entry.script.clone());
                // A reply with words is another choice. One without is said on the way.
                for r in entry.links.iter().map(|l| l.index as usize) {
                    if self.node_text(&d.replies[r]).is_empty() {
                        todo.push(r);
                    } else {
                        choice_next = true;
                    }
                }
            }
        }
        scripts.retain(|s| !s.is_empty());
        Some((scripts, choice_next))
    }

    /// Whether reply `k` does anything more than end the talk: a script, or another choice. A wrong answer to Zhar's Jedi
    /// Code is told so and sent away, and a right one goes on to the next line.
    pub fn reply_leads_on(&self, k: usize) -> bool {
        self.what_reply_leads_to(k)
            .is_some_and(|(scripts, choice_next)| choice_next || !scripts.is_empty())
    }

    /// Whether reply `k` of the held line sets someone who cannot be hurt on the player: a fight it cannot win, as
    /// `k_ptar_killpc` starts with the Sith guard at the undercity door. Conditions on the way are not weighed.
    ///
    /// **Assumption:** this is what such replies have in common. A foe that can be hurt is a fight, not a death.
    pub fn reply_is_fatal(&mut self, k: usize) -> bool {
        let Some(run) = self.conv.as_ref() else {
            return false;
        };
        if !self.object(run.owner).is_some_and(|o| o.plot) {
            return false;
        }
        let Some((scripts, _)) = self.what_reply_leads_to(k) else {
            return false;
        };
        let Some(turn) = self.routine_id("ChangeToStandardFaction") else {
            return false;
        };
        scripts.iter().any(|s| {
            self.program(s)
                .is_some_and(|p| p.reachable_routines().contains(&turn))
        })
    }

    /// Who a held conversation belongs to.
    pub fn dialogue_owner(&self) -> Option<ObjId> {
        self.conv.as_ref().map(|c| c.owner)
    }

    /// Whether an interactive conversation is waiting for the player.
    pub fn in_dialogue(&self) -> bool {
        self.conv.is_some()
    }

    /// Answer a held conversation: reply number `k` (ignored when the line only needs acknowledging).
    pub fn dialogue_choose(&mut self, k: usize) {
        if self.conversation_paused() {
            return;
        }
        let Some(mut run) = self.conv.take() else {
            return;
        };
        if self.conv_drive(&mut run, Some(k), None) {
            self.conv_finish(run);
        } else {
            self.conv = Some(run);
        }
    }

    fn conv_begin(&mut self, owner: ObjId, dialogue: &str) -> Result<ConvRun, String> {
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
        let mut d = d;
        self.add_story_branches(&name, &mut d);
        let before = self.transcript.len();
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
        Ok(ConvRun {
            name,
            owner,
            camera_model,
            stunts,
            dialog: d,
            next,
            visited: 0,
            before,
            steps: Vec::new(),
            held: None,
            waiting: false,
        })
    }

    /// Run the conversation until it ends (`true`) or, with no `chooser`, until it needs the player (`false`).
    /// `resume` answers the held line first.
    fn conv_drive(
        &mut self,
        run: &mut ConvRun,
        mut resume: Option<usize>,
        mut chooser: Option<&mut dyn Chooser>,
    ) -> bool {
        let (owner, name) = (run.owner, run.name.clone());
        loop {
            // A live conversation waits while an `ActionPauseConversation` is in force: someone is walking somewhere.
            if chooser.is_none()
                && run.held.is_none()
                && run.next.is_some()
                && self.conversation_paused()
            {
                run.waiting = true;
                return false;
            }
            run.waiting = false;
            let (options, shown, k): (Vec<(usize, String)>, Vec<String>, usize) =
                if let Some(h) = run.held.take() {
                    (h.options, h.shown, resume.take().unwrap_or(0))
                } else {
                    let Some(ei) = run.next.take() else {
                        return true;
                    };
                    run.visited += 1;
                    if run.visited > MAX_NODES {
                        self.errors
                            .push(format!("dialogue {name}: stopped after {MAX_NODES} nodes"));
                        return true;
                    }
                    let entry = run.dialog.entries[ei].clone();
                    let ev0 = self.events.len();
                    self.run_node_script(&entry, owner);
                    let text = self.node_text(&entry);
                    let speaker = self.speaker_name(&entry, owner);
                    let step = crate::cutscene::step_of(
                        &entry,
                        LineKind::Npc,
                        ei,
                        speaker.clone(),
                        text.clone(),
                        &self.events[ev0..],
                    );
                    let shown_step = step.clone();
                    let vo = if step.vo.is_empty() {
                        step.sound.clone()
                    } else {
                        step.vo.clone()
                    };
                    run.steps.push(step);
                    if !text.is_empty() {
                        self.transcript.push(Line {
                            speaker: speaker.clone(),
                            text: text.clone(),
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
                            options.push((r, self.node_text(&run.dialog.replies[r])));
                        }
                    }
                    if options.is_empty() {
                        return true;
                    }
                    // Continue links (no text) are followed automatically; otherwise the player picks among the texts.
                    let shown: Vec<String> = options
                        .iter()
                        .filter(|(_, t)| !t.is_empty())
                        .map(|(_, t)| t.clone())
                        .collect();
                    match chooser.as_mut() {
                        Some(c) => {
                            let k = if shown.is_empty() {
                                0
                            } else {
                                c.choose(&shown)
                            };
                            (options, shown, k)
                        }
                        None if text.is_empty() && shown.is_empty() && !presents(&shown_step) => {
                            (options, shown, 0)
                        }
                        None => {
                            run.held = Some(Held {
                                speaker_tag: entry.speaker.clone().unwrap_or_default(),
                                listener_tag: entry.listener.clone().unwrap_or_default(),
                                speaker,
                                text,
                                vo,
                                step: shown_step,
                                options,
                                shown,
                            });
                            return false;
                        }
                    }
                };
            let pick = if shown.is_empty() {
                0
            } else {
                let k = k.min(shown.len() - 1);
                options
                    .iter()
                    .position(|(_, t)| *t == shown[k])
                    .unwrap_or(0)
            };
            let (ri, rtext) = options[pick].clone();
            let reply = run.dialog.replies[ri].clone();
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
            self.run_node_script(&reply, owner);
            run.steps.push(crate::cutscene::step_of(
                &reply,
                LineKind::Player,
                ri,
                self.describe_short(self.pc),
                rtext,
                &self.events[ev1..],
            ));
            for l in &reply.links {
                if self.check(&l.active, owner) {
                    run.next = Some(l.index as usize);
                    break;
                }
            }
        }
    }

    fn conv_finish(&mut self, run: ConvRun) {
        let (owner, name, d) = (run.owner, run.name, run.dialog);
        // Scripts that end a conversation.
        let end_script = d.extra.iter().find_map(|(l, v)| match (l.as_str(), v) {
            ("EndConversation", Value::ResRef(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        });
        if let Some(s) = end_script {
            let _ = self.run_script(&s, owner);
        }
        self.events.push(format!(
            "conversation {name} ends after {} lines, {} nodes",
            self.transcript.len() - run.before,
            run.visited
        ));
        self.in_conversation = false;
        self.conversation = Some(crate::cutscene::Conversation {
            name,
            camera_model: run.camera_model,
            stunts: run.stunts,
            steps: run.steps,
        });
    }
}

/// A wordless node still has to be seen when it moves the camera or a body, or asks for time.
fn presents(s: &crate::cutscene::Step) -> bool {
    s.camera_animation.is_some()
        || s.camera_id.is_some()
        || s.delay.is_some()
        || !s.animations.is_empty()
}

/// A conversation in progress.
pub struct ConvRun {
    name: String,
    owner: ObjId,
    camera_model: String,
    stunts: Vec<(String, String)>,
    dialog: Dialog,
    next: Option<usize>,
    visited: usize,
    before: usize,
    steps: Vec<crate::cutscene::Step>,
    held: Option<Held>,
    /// Stopped between nodes for a pause.
    waiting: bool,
}

/// A spoken line waiting for the player.
struct Held {
    speaker_tag: String,
    listener_tag: String,
    speaker: String,
    text: String,
    vo: String,
    step: crate::cutscene::Step,
    options: Vec<(usize, String)>,
    shown: Vec<String>,
}
