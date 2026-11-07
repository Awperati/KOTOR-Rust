//! The owner's own animations: `*.anim.txt` files, made with the Blender add-on (`tools/blender/kotor_anim.py`), that the game
//! plays in place of a stock clip of the same name or as a clip of their own.
//!
//! ```text
//! version = 2
//! name = wave
//! fps = 30
//! loop = yes
//! key rforearm_g 0 0.0 0.0 0.27  1 0 0 0
//! key rforearm_g 5 0.0 0.0 0.27  0.92 0.38 0 0
//! ```
//!
//! A `key` line is a bone's name as the game has it (lower case), a frame number (at `fps` frames a second), the bone's place
//! relative to its parent in metres, and its turn relative to its parent as a quaternion `w x y z`: what the exporter gives,
//! so a bone is as the game has it in the file. Between two keys a place is blended in a line and a turn along the shorter arc.
//! A bone with no key stays as the body rests. The root's sideways and forward movement is not played, since the game moves
//! the body; its height is.

use kotor_model::{blend_by, NodePose, Pose};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// The version of the file this reads.
pub const VERSION: u32 = 2;

/// One key of a bone: its frame, place and turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub frame: f32,
    pub position: [f32; 3],
    pub orientation: [f32; 4],
}

/// A clip of the owner's.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomClip {
    /// Lower case, as the game names a clip.
    pub name: String,
    pub fps: f32,
    pub looping: bool,
    /// Seconds, to the last key.
    pub length: f32,
    /// Each bone's keys by frame.
    pub tracks: HashMap<String, Vec<Key>>,
}

/// Nodes whose forward and sideways movement the game does itself.
const ROOT_NODES: [&str; 3] = ["rootdummy", "cutscenedummy", "pelvis_g"];

impl CustomClip {
    /// Read a clip from the text of a file. `Err` holds every mistake found, each with its line.
    pub fn parse(text: &str) -> Result<CustomClip, Vec<String>> {
        let mut errors = Vec::new();
        let (mut version, mut name, mut fps, mut looping) = (None, None, 30.0f32, true);
        let mut tracks: HashMap<String, Vec<Key>> = HashMap::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let at = n + 1;
            if let Some(rest) = line.strip_prefix("key ") {
                let f: Vec<&str> = rest.split_whitespace().collect();
                if f.len() != 9 {
                    errors.push(format!("line {at}: a key is `key <bone> <frame> <x> <y> <z> <w> <qx> <qy> <qz>`, nine words"));
                    continue;
                }
                let nums: Vec<Option<f32>> = f[1..].iter().map(|v| v.parse::<f32>().ok()).collect();
                if nums.iter().any(|v| v.is_none_or(|x| !x.is_finite())) {
                    errors.push(format!("line {at}: a number is wrong in `{line}`"));
                    continue;
                }
                let v: Vec<f32> = nums.into_iter().flatten().collect();
                let norm = v[4..8].iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm < 1e-3 {
                    errors.push(format!("line {at}: the turn of {} is all zeros", f[0]));
                    continue;
                }
                if v[0] < 0.0 {
                    errors.push(format!("line {at}: frame {} is before the start", v[0]));
                    continue;
                }
                tracks
                    .entry(f[0].to_ascii_lowercase())
                    .or_default()
                    .push(Key {
                        frame: v[0],
                        position: [v[1], v[2], v[3]],
                        orientation: [v[4] / norm, v[5] / norm, v[6] / norm, v[7] / norm],
                    });
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                errors.push(format!(
                    "line {at}: `{line}` is not `key = value` or a `key` line"
                ));
                continue;
            };
            let (key, value) = (key.trim().to_ascii_lowercase(), value.trim());
            match key.as_str() {
                "version" => version = value.parse::<u32>().ok(),
                "name" => name = Some(value.to_ascii_lowercase()),
                "fps" => match value.parse::<f32>() {
                    Ok(f) if f > 0.0 && f <= 240.0 => fps = f,
                    _ => errors.push(format!("line {at}: fps must be a number from 1 to 240")),
                },
                "loop" => {
                    looping = matches!(value.to_ascii_lowercase().as_str(), "yes" | "true" | "1")
                }
                other => errors.push(format!("line {at}: unknown setting `{other}`")),
            }
        }
        if version != Some(VERSION) {
            errors.push(format!(
                "the file must say `version = {VERSION}` (it says {version:?})"
            ));
        }
        let name = name
            .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        if name.is_none() {
            errors.push("the file must say `name = <letters, digits and _>`".to_string());
        }
        if tracks.is_empty() {
            errors.push("the file has no `key` lines".to_string());
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let mut last = 0.0f32;
        for keys in tracks.values_mut() {
            keys.sort_by(|a, b| a.frame.total_cmp(&b.frame));
            last = last.max(keys.last().map_or(0.0, |k| k.frame));
        }
        Ok(CustomClip {
            name: name.unwrap_or_default(),
            fps,
            looping,
            length: (last / fps).max(1.0 / fps),
            tracks,
        })
    }

    /// The pose at `t` seconds: each keyed bone as the keys put it, the root's walking taken out (`rest` is the body's own).
    pub fn sample(&self, t: f32, rest: &Pose) -> Pose {
        let frame = if self.looping {
            (t * self.fps).rem_euclid((self.length * self.fps).max(1.0))
        } else {
            (t * self.fps).clamp(0.0, self.length * self.fps)
        };
        let mut pose = Pose::new();
        for (bone, keys) in &self.tracks {
            let at = match keys.iter().position(|k| k.frame > frame) {
                None => keys[keys.len() - 1],
                Some(0) => keys[0],
                Some(i) => {
                    let (a, b) = (keys[i - 1], keys[i]);
                    let w = (frame - a.frame) / (b.frame - a.frame).max(1e-6);
                    let one = |k: &Key| -> Pose {
                        [(
                            "x".to_string(),
                            NodePose {
                                position: k.position,
                                orientation: k.orientation,
                                alpha: None,
                            },
                        )]
                        .into_iter()
                        .collect()
                    };
                    let mixed = blend_by(&one(&a), &one(&b), w);
                    Key {
                        frame,
                        position: mixed["x"].position,
                        orientation: mixed["x"].orientation,
                    }
                }
            };
            let mut position = at.position;
            if ROOT_NODES.contains(&bone.as_str()) {
                if let Some(r) = rest.get(bone) {
                    position[0] = r.position[0];
                    position[1] = r.position[1];
                }
            }
            pose.insert(
                bone.clone(),
                NodePose {
                    position,
                    orientation: at.orientation,
                    alpha: None,
                },
            );
        }
        pose
    }
}

static REGISTRY: RwLock<Option<HashMap<String, Arc<CustomClip>>>> = RwLock::new(None);

/// Make `clips` the owner's clips: what `get` finds.
pub fn set_all(clips: Vec<CustomClip>) {
    let map = clips
        .into_iter()
        .map(|c| (c.name.clone(), Arc::new(c)))
        .collect();
    if let Ok(mut r) = REGISTRY.write() {
        *r = Some(map);
    }
}

/// The owner's clip of this name, if he has one (any case).
pub fn get(name: &str) -> Option<Arc<CustomClip>> {
    REGISTRY
        .read()
        .ok()?
        .as_ref()?
        .get(&name.to_ascii_lowercase())
        .cloned()
}

/// The names of the owner's clips, in order.
pub fn names() -> Vec<String> {
    let mut v: Vec<String> = REGISTRY
        .read()
        .ok()
        .and_then(|r| r.as_ref().map(|m| m.keys().cloned().collect()))
        .unwrap_or_default();
    v.sort();
    v
}

/// Read every `*.anim.txt` in `folder`. Returns the clips that read, and a line for each file or bone that is wrong: a file
/// that does not read is left out; a bone the body does not have (`bones` lists the body's) is named and ignored.
pub fn load_folder(folder: &Path, bones: &[String]) -> (Vec<CustomClip>, Vec<String>) {
    let (mut clips, mut problems) = (Vec::new(), Vec::new());
    let Ok(dir) = std::fs::read_dir(folder) else {
        return (clips, problems);
    };
    let mut files: Vec<_> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.to_ascii_lowercase().ends_with(".anim.txt"))
        })
        .collect();
    files.sort();
    for path in files {
        let shown = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                problems.push(format!("{shown}: cannot be read ({e})"));
                continue;
            }
        };
        match CustomClip::parse(&text) {
            Ok(clip) => {
                let mut extra: Vec<&String> = clip
                    .tracks
                    .keys()
                    .filter(|b| !bones.is_empty() && !bones.iter().any(|k| k == *b))
                    .collect();
                extra.sort();
                if !extra.is_empty() {
                    problems.push(format!(
                        "{shown}: the body has no bone named {}; those keys are ignored",
                        extra
                            .iter()
                            .map(|b| b.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                clips.push(clip);
            }
            Err(errs) => {
                for e in errs {
                    problems.push(format!("{shown}: {e}; the game is not using this file"));
                }
            }
        }
    }
    (clips, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAVE: &str = "version = 2\nname = Wave\nfps = 30\nloop = yes\n# a wave\nkey rforearm_g 0 0 0 0.27 1 0 0 0\nkey rforearm_g 10 0 0 0.27 0.7071 0.7071 0 0\nkey rootdummy 0 5 6 1.0 1 0 0 0\n";

    #[test]
    fn a_file_reads_and_samples_between_its_keys() {
        let c = CustomClip::parse(WAVE).unwrap();
        assert_eq!(c.name, "wave");
        assert!((c.length - 10.0 / 30.0).abs() < 1e-5);
        let rest: Pose = [(
            "rootdummy".to_string(),
            NodePose {
                position: [0.0, 0.0, 1.0],
                orientation: [1.0, 0.0, 0.0, 0.0],
                alpha: None,
            },
        )]
        .into_iter()
        .collect();
        let mid = c.sample(5.0 / 30.0, &rest);
        let q = mid["rforearm_g"].orientation;
        assert!(
            (q[0] - 0.9239).abs() < 0.01 && (q[1] - 0.3827).abs() < 0.01,
            "{q:?}"
        );
        // The root's walking is not played, its height is.
        assert_eq!(mid["rootdummy"].position, [0.0, 0.0, 1.0]);
    }

    /// The worked example of docs/BLENDER_ANIMATION.md, written by the add-on's own `format_clip` (tools/blender/kotor_anim.py):
    /// what the add-on writes is what the engine reads, and it plays as a wave of eleven frames.
    #[test]
    fn what_the_blender_add_on_writes_the_engine_reads() {
        let text = include_str!("../../../docs/examples/wave.anim.txt");
        let c = CustomClip::parse(text).unwrap();
        assert_eq!(c.name, "wave");
        assert_eq!(c.fps, 30.0);
        assert!(c.looping);
        assert!((c.length - 10.0 / 30.0).abs() < 1e-5);
        assert_eq!(c.tracks.len(), 2);
        assert_eq!(c.tracks["rbicep_g"].len(), 11);
        let none = Pose::new();
        let (a, b) = (c.sample(0.0, &none), c.sample(3.0 / 30.0, &none));
        assert_ne!(
            a["rforearm_g"].orientation, b["rforearm_g"].orientation,
            "the forearm sways"
        );
        assert_ne!(
            a["rbicep_g"].orientation, b["rbicep_g"].orientation,
            "the arm goes up"
        );
        // Looped: a whole length on is the start.
        let again = c.sample(c.length, &none);
        for k in 0..4 {
            assert!(
                (again["rforearm_g"].orientation[k] - a["rforearm_g"].orientation[k]).abs() < 1e-4
            );
        }
    }

    #[test]
    fn a_wrong_file_says_what_is_wrong() {
        let errs =
            CustomClip::parse("version = 1\nname = x\nkey a 0 0 0 0 1 0 0\nbogus\n").unwrap_err();
        let all = errs.join("\n");
        assert!(all.contains("version = 2"), "{all}");
        assert!(all.contains("nine words"), "{all}");
        assert!(all.contains("not `key = value`"), "{all}");
    }
}
