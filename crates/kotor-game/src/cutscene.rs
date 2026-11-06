//! Cutscene sequencing: a played conversation turned into a timeline of beats with a start time, a length, a camera
//! shot, animation cues and the holds that `ActionPauseConversation` makes.
//!
//! **Read from the install** (per DLG node): `AnimList` (participant tag and animation id), `CameraAnimation`,
//! `CameraAngle`, `CameraID`, `CamFieldOfView`, `Delay`, `WaitFlags`, `FadeType`, `VO_ResRef`, `Sound`, the
//! dialogue's `CameraModel` and `StuntList` (participant tag -> stunt model). Lengths come from the stunt models'
//! own animations and from the voice-over file's measured duration.
//!
//! **Assumed, not measured from the game (I do not know these):**
//! - Animation ids 1200 and up index the stunt model's animations in file order (the Endar Spire opening has 18 ids,
//!   1200-1217, and its stunt models have exactly 18 animations, `CUT001W` ... `CUT021W`). Ids outside that range
//!   (1219 and 1220 appear for the player) are not resolved and contribute no length.
//! - A beat lasts as long as its longest camera animation, participant animation or voice-over; with none of those
//!   it lasts `Delay` seconds if the node has one, else the time to read its text at 15 characters a second
//!   (at least 1.5 s), else 0. A `Delay` longer than that stretches the beat. `WaitFlags` is recorded but not used.
//! - `ActionPauseConversation` holds the conversation after its node until the matching `ActionResumeConversation`;
//!   the hold is the sum of the `ActionWait` seconds queued in between. Movement and animation lengths inside the
//!   paused action list are not modelled.
//! - Scripts run when the conversation is resolved (as before), not at their beat's start time.

use crate::dialogue::LineKind;
use kotor_gff::dlg::Node;
use kotor_gff::Value;

/// One node of a played conversation, with the presentation data of its DLG node.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub kind: LineKind,
    pub node: usize,
    pub speaker: String,
    pub text: String,
    pub vo: String,
    pub sound: String,
    pub script: String,
    /// (participant tag, animation id).
    pub animations: Vec<(String, u32)>,
    pub camera_angle: Option<u32>,
    pub camera_id: Option<i32>,
    pub camera_animation: Option<u32>,
    pub field_of_view: Option<f32>,
    /// Seconds, when the node sets one.
    pub delay: Option<f32>,
    pub wait_flags: u32,
    pub fade_type: u32,
    /// What the node's script recorded while it ran (`ActionPauseConversation()`, `ActionWait(1.5)`, ...).
    pub events: Vec<String>,
}

/// A played conversation with what the sequencer needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Conversation {
    pub name: String,
    pub camera_model: String,
    /// (participant tag, stunt model name).
    pub stunts: Vec<(String, String)>,
    pub steps: Vec<Step>,
}

fn num(extra: &[(String, Value)], label: &str) -> Option<f64> {
    extra.iter().find(|(l, _)| l == label).and_then(|(_, v)| {
        Some(match v {
            Value::Byte(x) => *x as f64,
            Value::Word(x) => *x as f64,
            Value::Dword(x) => *x as f64,
            Value::Short(x) => *x as f64,
            Value::Int(x) => *x as f64,
            Value::Float(x) => *x as f64,
            _ => return None,
        })
    })
}

fn string(extra: &[(String, Value)], label: &str) -> String {
    extra
        .iter()
        .find_map(|(l, v)| match (l.as_str(), v) {
            (x, Value::ResRef(s) | Value::String(s)) if x == label => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Read the presentation fields of a node.
pub(crate) fn step_of(
    n: &Node,
    kind: LineKind,
    index: usize,
    speaker: String,
    text: String,
    events: &[String],
) -> Step {
    let e = &n.extra;
    let animations = e
        .iter()
        .find_map(|(l, v)| match (l.as_str(), v) {
            ("AnimList", Value::List(items)) => Some(
                items
                    .iter()
                    .filter_map(|s| {
                        Some((
                            s.str("Participant")?.to_string(),
                            s.int("Animation")? as u32,
                        ))
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    let delay = num(e, "Delay")
        .filter(|d| *d >= 0.0 && *d < 4_000_000_000.0)
        .map(|d| d as f32);
    Step {
        kind,
        node: index,
        speaker,
        text,
        vo: n.vo.clone().unwrap_or_default(),
        sound: string(e, "Sound"),
        script: n.script.clone().unwrap_or_default(),
        animations,
        camera_angle: num(e, "CameraAngle").map(|v| v as u32),
        camera_id: num(e, "CameraID").map(|v| v as i32).filter(|v| *v >= 0),
        camera_animation: num(e, "CameraAnimation").map(|v| v as u32),
        field_of_view: num(e, "CamFieldOfView").map(|v| v as f32),
        delay,
        wait_flags: num(e, "WaitFlags").unwrap_or(0.0) as u32,
        fade_type: num(e, "FadeType").unwrap_or(0.0) as u32,
        events: events.to_vec(),
    }
}

/// Where the lengths come from; the world supplies one backed by the install, tests supply fixed ones.
pub trait Lookup {
    /// Length of animation `id` on the conversation's camera model.
    fn camera_animation_seconds(&mut self, id: u32) -> Option<f32>;
    /// Length of animation `id` on the stunt model of `participant`.
    fn participant_animation_seconds(&mut self, participant: &str, id: u32) -> Option<f32>;
    /// Length of a voice-over file.
    fn voice_seconds(&mut self, resref: &str) -> Option<f32>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum CameraShot {
    /// The conversation's camera model plays animation `animation` (`seconds` long).
    Animated {
        animation: u32,
        seconds: Option<f32>,
        fov: Option<f32>,
    },
    /// A fixed camera object of the area.
    Static { id: i32, fov: Option<f32> },
    /// One of the engine's standard angles, by code (their meaning is not known here).
    Angle { code: u32, fov: Option<f32> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LengthFrom {
    CameraAnimation,
    ParticipantAnimation,
    Voice,
    Delay,
    Text,
    Nothing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Beat {
    pub step: Step,
    pub start: f32,
    pub length: f32,
    pub from: LengthFrom,
    pub camera: Option<CameraShot>,
    /// Seconds the conversation waits after this beat for a paused action list (see the module notes).
    pub hold: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub name: String,
    pub beats: Vec<Beat>,
    pub total: f32,
}

/// A line with no voice-over is held for `READING_SECONDS_PER_CHAR` a character, at least 2 s and at most 8 s (the owner's rule,
/// 6 Nov 2026).
const READING_SECONDS_PER_CHAR: f32 = 0.06;
const MINIMUM_READING_SECONDS: f32 = 2.0;
const MAXIMUM_READING_SECONDS: f32 = 8.0;

/// Seconds a pause holds for: the `ActionWait` amounts recorded between a pause and its resume, where
/// `pending` carries a pause still open from earlier nodes.
fn hold_of(events: &[String], pending: &mut Option<f32>) -> f32 {
    let mut closed = 0.0;
    for e in events {
        if e.starts_with("ActionPauseConversation") {
            *pending = Some(0.0);
        } else if let Some(w) = pending.as_mut() {
            if let Some(arg) = e
                .strip_prefix("ActionWait(")
                .and_then(|a| a.strip_suffix(')'))
            {
                *w += arg.trim().parse::<f32>().unwrap_or(0.0);
            } else if e.starts_with("ActionResumeConversation") {
                closed += *w;
                *pending = None;
            }
        }
    }
    closed
}

impl Timeline {
    pub fn build(c: &Conversation, lookup: &mut dyn Lookup) -> Timeline {
        let mut beats = Vec::new();
        let (mut now, mut open_pause) = (0.0f32, None);
        for s in &c.steps {
            let cam = s
                .camera_animation
                .and_then(|id| lookup.camera_animation_seconds(id));
            let part = s
                .animations
                .iter()
                .filter_map(|(who, id)| lookup.participant_animation_seconds(who, *id))
                .fold(None, |a: Option<f32>, b| Some(a.map_or(b, |a| a.max(b))));
            let voice = (!s.vo.is_empty())
                .then(|| lookup.voice_seconds(&s.vo))
                .flatten();
            let candidates = [
                (cam, LengthFrom::CameraAnimation),
                (part, LengthFrom::ParticipantAnimation),
                (voice, LengthFrom::Voice),
            ];
            // The longest wins; on a tie the earlier source (camera, then participants, then voice) is named.
            let best = candidates
                .iter()
                .filter_map(|(v, f)| v.map(|v| (v, *f)))
                .fold(None, |best: Option<(f32, LengthFrom)>, c| match best {
                    Some(b) if b.0 >= c.0 => Some(b),
                    _ => Some(c),
                });
            let (mut length, mut from) = match best {
                Some((v, f)) => (v, f),
                None => match (s.delay, s.text.is_empty()) {
                    (Some(d), _) => (d, LengthFrom::Delay),
                    (None, false) => (
                        (s.text.chars().count() as f32 * READING_SECONDS_PER_CHAR)
                            .clamp(MINIMUM_READING_SECONDS, MAXIMUM_READING_SECONDS),
                        LengthFrom::Text,
                    ),
                    (None, true) => (0.0, LengthFrom::Nothing),
                },
            };
            if let Some(d) = s.delay {
                if d > length {
                    length = d;
                    from = LengthFrom::Delay;
                }
            }
            let camera = match (s.camera_animation, s.camera_id, s.camera_angle) {
                (Some(a), _, Some(4)) | (Some(a), None, None) => Some(CameraShot::Animated {
                    animation: a,
                    seconds: cam,
                    fov: s.field_of_view,
                }),
                (_, Some(id), _) => Some(CameraShot::Static {
                    id,
                    fov: s.field_of_view,
                }),
                (_, None, Some(code)) => Some(CameraShot::Angle {
                    code,
                    fov: s.field_of_view,
                }),
                _ => None,
            };
            let hold = hold_of(&s.events, &mut open_pause);
            beats.push(Beat {
                step: s.clone(),
                start: now,
                length,
                from,
                camera,
                hold,
            });
            now += length + hold;
        }
        Timeline {
            name: c.name.clone(),
            beats,
            total: now,
        }
    }

    /// The beat playing at time `t` (the last one once the cutscene is over).
    pub fn beat_at(&self, t: f32) -> Option<&Beat> {
        self.beats
            .iter()
            .rev()
            .find(|b| b.start <= t)
            .or(self.beats.first())
    }
}

/// Plays a timeline against a clock: advance it, pause it, skip it.
#[derive(Clone, Debug)]
pub struct Player {
    pub timeline: Timeline,
    pub time: f32,
    pub paused: bool,
    /// Index of the next beat to announce.
    next: usize,
}

impl Player {
    pub fn new(timeline: Timeline) -> Player {
        Player {
            timeline,
            time: 0.0,
            paused: false,
            next: 0,
        }
    }

    pub fn finished(&self) -> bool {
        self.time >= self.timeline.total
    }

    /// Move the clock on by `dt` (nothing while paused) and return the beats that started in that time, in order.
    pub fn advance(&mut self, dt: f32) -> Vec<&Beat> {
        if self.paused || self.finished() {
            return Vec::new();
        }
        self.time = (self.time + dt).min(self.timeline.total);
        let from = self.next;
        while self
            .timeline
            .beats
            .get(self.next)
            .is_some_and(|b| b.start <= self.time)
        {
            self.next += 1;
        }
        self.timeline.beats[from..self.next].iter().collect()
    }

    /// Jump to the end (the player skips the cutscene); returns the beats not yet announced.
    pub fn skip(&mut self) -> Vec<&Beat> {
        let from = self.next;
        self.time = self.timeline.total;
        self.next = self.timeline.beats.len();
        self.timeline.beats[from..].iter().collect()
    }

    pub fn current(&self) -> Option<&Beat> {
        self.timeline.beat_at(self.time)
    }
}

use crate::world::World;
use kotor_core::ResType;
use std::collections::HashMap;

/// First animation id that indexes a stunt model's animations (assumption, see `cutscene.rs`).
const FIRST_STUNT_ANIMATION: u32 = 1200;

struct WorldLookup<'a> {
    world: &'a mut World,
    conv: &'a Conversation,
    /// Animation lengths per model, in file order.
    models: HashMap<String, Vec<f32>>,
}

impl WorldLookup<'_> {
    fn lengths(&mut self, model: &str) -> &Vec<f32> {
        let key = model.to_ascii_lowercase();
        if !self.models.contains_key(&key) {
            let mdl = self.world.inst.read(&key, ResType::MDL).ok();
            let mdx = self
                .world
                .inst
                .read(&key, ResType(3008))
                .unwrap_or_default();
            let lens = mdl
                .and_then(|m| kotor_model::parse_model(&m, &mdx).ok())
                .map(|m| m.animations.iter().map(|a| a.length).collect())
                .unwrap_or_default();
            self.models.insert(key.clone(), lens);
        }
        &self.models[&key]
    }

    fn nth(&mut self, model: &str, id: u32) -> Option<f32> {
        let i = id.checked_sub(FIRST_STUNT_ANIMATION)? as usize;
        self.lengths(model).get(i).copied()
    }
}

impl Lookup for WorldLookup<'_> {
    fn camera_animation_seconds(&mut self, id: u32) -> Option<f32> {
        let model = self.conv.camera_model.clone();
        (!model.is_empty()).then(|| self.nth(&model, id)).flatten()
    }

    fn participant_animation_seconds(&mut self, participant: &str, id: u32) -> Option<f32> {
        let model = self
            .conv
            .stunts
            .iter()
            .find(|(p, _)| p.eq_ignore_ascii_case(participant))
            .map(|(_, m)| m.clone())?;
        self.nth(&model, id)
    }

    fn voice_seconds(&mut self, resref: &str) -> Option<f32> {
        let root = self.world.inst.dir().to_path_buf();
        ["streamwaves", "streamvoice", "streamsounds"]
            .iter()
            .find_map(|sub| {
                let bytes = std::fs::read(root.join(sub).join(format!("{resref}.wav"))).ok()?;
                kotor_audio::probe(&bytes)
                    .ok()
                    .map(|i| i.duration_secs as f32)
            })
    }
}

impl World {
    /// The beat of the node a held conversation is waiting on.
    pub fn dialogue_beat(&mut self) -> Option<Beat> {
        let conv = self.dialogue_conversation()?;
        let mut lookup = WorldLookup {
            world: self,
            conv: &conv,
            models: HashMap::new(),
        };
        Timeline::build(&conv, &mut lookup).beats.pop()
    }

    /// The timeline of the conversation played last.
    pub fn cutscene_timeline(&mut self) -> Option<Timeline> {
        let conv = self.conversation.clone()?;
        let mut lookup = WorldLookup {
            world: self,
            conv: &conv,
            models: HashMap::new(),
        };
        Some(Timeline::build(&conv, &mut lookup))
    }
}
