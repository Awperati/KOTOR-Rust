//! The roll's and the landing's poses as plain text the owner can edit: keys through time, each giving joints their angles.
//! See `docs/MOVEMENT_POSE.md`. This file reads the text and says what the joints are at a moment. `moving.rs` turns them.
//!
//! **Assumed:** the format is mine, a small one with no new library. A mistake anywhere in a file means the whole default
//! is used, not a part of the file, so that a half read pose is never drawn.

/// Which side of the body a joint is on, so that a positive `out` or `twist` means away from the body on both.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Side {
    Left,
    Right,
    Middle,
}

/// A joint the files may name.
#[derive(Clone, Copy)]
pub(super) struct Joint {
    pub name: &'static str,
    pub side: Side,
    /// Which way about the side to side line is a positive `bend`: the way this joint curls in.
    pub flex: f32,
    /// The joint its limb reaches for: `twist` turns about that line. None: about the body's up and down line.
    pub reaches: Option<&'static str>,
}

const fn joint(name: &'static str, side: Side, flex: f32, reaches: Option<&'static str>) -> Joint {
    Joint {
        name,
        side,
        flex,
        reaches,
    }
}

const COUNT: usize = 20;
pub(super) const JOINTS: [Joint; COUNT] = [
    joint("torso_g", Side::Middle, -1.0, None),
    joint("torsoupr_g", Side::Middle, -1.0, None),
    joint("necklwr_g", Side::Middle, -1.0, None),
    joint("neck_g", Side::Middle, -1.0, None),
    joint("head_g", Side::Middle, -1.0, None),
    joint("lthigh_g", Side::Left, 1.0, Some("lshin_g")),
    joint("rthigh_g", Side::Right, 1.0, Some("rshin_g")),
    joint("lshin_g", Side::Left, -1.0, Some("lfoot_g")),
    joint("rshin_g", Side::Right, -1.0, Some("rfoot_g")),
    joint("lfoot_g", Side::Left, 1.0, None),
    joint("rfoot_g", Side::Right, 1.0, None),
    joint("lcollar_g", Side::Left, 1.0, Some("lbicep_g")),
    joint("rcollar_g", Side::Right, 1.0, Some("rbicep_g")),
    joint("lbicep_g", Side::Left, 1.0, Some("lforearm_g")),
    joint("rbicep_g", Side::Right, 1.0, Some("rforearm_g")),
    joint("lforearm_g", Side::Left, 1.0, Some("lhand_g")),
    joint("rforearm_g", Side::Right, 1.0, Some("rhand_g")),
    joint("lhand_g", Side::Left, 1.0, None),
    joint("rhand_g", Side::Right, 1.0, None),
    joint("pelvis_g", Side::Middle, 1.0, None),
];

/// The text of the files as they come. The game writes them out the first time, and the owner's own stay.
pub(super) const DEFAULT_ROLL: &str = include_str!("roll_pose.txt");
pub(super) const DEFAULT_LANDING: &str = include_str!("landing_pose.txt");
pub(super) const DEFAULT_JUMP: &str = include_str!("jump_pose.txt");
/// What the game has written before, as it came: a file exactly like one of these was never edited, and gets the new default.
/// When a default changes, its old text goes in `poses_old` and in this list, and its `version` goes up.
pub(super) const SHIPPED_ROLLS: [&str; 2] = [
    include_str!("poses_old/roll_pose_v1.txt"),
    include_str!("poses_old/roll_pose_v2.txt"),
];
pub(super) const SHIPPED_LANDINGS: [&str; 1] = [include_str!("poses_old/landing_pose_v1.txt")];
pub(super) const SHIPPED_JUMPS: [&str; 4] = [
    include_str!("poses_old/jump_pose_v1.txt"),
    include_str!("poses_old/jump_pose_v2.txt"),
    include_str!("poses_old/jump_pose_v3.txt"),
    include_str!("poses_old/jump_pose_v4.txt"),
];
pub(super) const ROLL_FILE: &str = "roll_pose.txt";
pub(super) const LANDING_FILE: &str = "landing_pose.txt";
pub(super) const JUMP_FILE: &str = "jump_pose.txt";

#[derive(Clone, Copy, PartialEq, Debug)]
enum Ease {
    Linear,
    Smooth,
    In,
    Out,
}

impl Ease {
    fn apply(self, x: f32) -> f32 {
        match self {
            Ease::Linear => x,
            Ease::Smooth => x * x * (3.0 - 2.0 * x),
            Ease::In => x * x,
            Ease::Out => 1.0 - (1.0 - x) * (1.0 - x),
        }
    }
}

#[derive(Clone, Debug)]
struct Key {
    at: f32,
    ease: Ease,
    tuck: f32,
    lower: f32,
    angles: [[f32; 3]; COUNT],
}

/// One file, read.
#[derive(Clone, Debug)]
pub(super) struct PoseFile {
    /// Which of the game's own defaults this began as. A file from before there were versions is 0.
    pub version: u32,
    /// Whole somersaults over the length of the move.
    pub turns: f32,
    keys: Vec<Key>,
}

/// The pose at a moment.
pub(super) struct Sampled {
    pub tuck: f32,
    pub lower: f32,
    /// Bend, out and twist in degrees, in the order of `JOINTS`.
    pub angles: [[f32; 3]; COUNT],
}

impl Sampled {
    /// This moment on its way to another, `share` of the way there.
    pub fn toward(&self, other: &Sampled, share: f32) -> Sampled {
        let mix = |a: f32, b: f32| a + (b - a) * share;
        let mut angles = self.angles;
        for (a, b) in angles.iter_mut().zip(&other.angles) {
            for k in 0..3 {
                a[k] = mix(a[k], b[k]);
            }
        }
        Sampled {
            tuck: mix(self.tuck, other.tuck),
            lower: mix(self.lower, other.lower),
            angles,
        }
    }
}

fn number(words: &str, line: usize) -> Result<f32, String> {
    match words.trim().parse::<f32>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => Err(format!("line {line}: \"{}\" is not a number", words.trim())),
    }
}

fn within(v: f32, low: f32, high: f32, what: &str, line: usize) -> Result<f32, String> {
    if v < low || v > high {
        Err(format!(
            "line {line}: {what} is {v}, and it must be from {low} to {high}"
        ))
    } else {
        Ok(v)
    }
}

/// The same text, whatever Notepad or git did to its first mark and its line ends.
pub(super) fn same_text(a: &str, b: &str) -> bool {
    let plain = |t: &str| t.trim_start_matches('\u{feff}').replace('\r', "");
    plain(a) == plain(b)
}

impl PoseFile {
    /// The text of a file, or what is wrong with it, with its line.
    pub fn parse(text: &str) -> Result<PoseFile, String> {
        let mut file = PoseFile {
            version: 0,
            turns: 0.0,
            keys: Vec::new(),
        };
        // `at` must be said in each key.
        let mut said_at = true;
        for (i, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            let line = i + 1;
            let words = raw.split('#').next().unwrap_or("").trim();
            if words.is_empty() {
                continue;
            }
            if words.eq_ignore_ascii_case("[key]") {
                if !said_at {
                    return Err(format!("line {line}: the key before this has no \"at\""));
                }
                said_at = false;
                file.keys.push(Key {
                    at: 0.0,
                    ease: Ease::Smooth,
                    tuck: 0.0,
                    lower: 0.0,
                    angles: [[0.0; 3]; COUNT],
                });
                continue;
            }
            let Some((name, value)) = words.split_once('=') else {
                return Err(format!(
                    "line {line}: \"{words}\" has no \"=\", and is not [key]"
                ));
            };
            let name = name.trim().to_ascii_lowercase();
            let Some(key) = file.keys.last_mut() else {
                match name.as_str() {
                    "version" => {
                        let v = within(number(value, line)?, 0.0, 1000.0, "version", line)?;
                        file.version = v as u32;
                    }
                    "turns" => {
                        let v = within(number(value, line)?, -3.0, 3.0, "turns", line)?;
                        if v.fract() != 0.0 {
                            return Err(format!(
                                "line {line}: turns is {v}, and it must be a whole number or the body jumps at the end"
                            ));
                        }
                        file.turns = v;
                    }
                    // In a file edited before the body found the floor by itself. It still reads.
                    "centre_height" => {}
                    _ => return Err(format!("line {line}: \"{name}\" must come after a [key]")),
                }
                continue;
            };
            match name.as_str() {
                "at" => {
                    key.at = within(number(value, line)?, 0.0, 1.0, "at", line)?;
                    said_at = true;
                }
                "ease" => {
                    key.ease = match value.trim().to_ascii_lowercase().as_str() {
                        "linear" => Ease::Linear,
                        "smooth" => Ease::Smooth,
                        "in" => Ease::In,
                        "out" => Ease::Out,
                        other => {
                            return Err(format!(
                                "line {line}: ease \"{other}\" must be linear, smooth, in or out"
                            ))
                        }
                    }
                }
                "tuck" => key.tuck = within(number(value, line)?, 0.0, 1.0, "tuck", line)?,
                "lower" => key.lower = within(number(value, line)?, 0.0, 1.0, "lower", line)?,
                _ => {
                    let Some(j) = JOINTS.iter().position(|j| j.name == name) else {
                        return Err(format!("line {line}: there is no joint \"{name}\""));
                    };
                    let values: Vec<&str> = value
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .filter(|w| !w.is_empty())
                        .collect();
                    if values.is_empty() || values.len() > 3 {
                        return Err(format!(
                            "line {line}: {name} takes one to three numbers: bend, out, twist"
                        ));
                    }
                    for (k, word) in values.iter().enumerate() {
                        let v = number(word, line)?;
                        key.angles[j][k] = within(v, -180.0, 180.0, &name, line)?;
                    }
                }
            }
        }
        if !said_at {
            return Err("the last key has no \"at\"".to_string());
        }
        if file.keys.is_empty() {
            return Err("there is no [key] in the file".to_string());
        }
        if file.keys.windows(2).any(|k| k[1].at <= k[0].at) {
            return Err("the keys' \"at\" must go up from one key to the next".to_string());
        }
        Ok(file)
    }

    /// The pose `p` of the way through the move, 0 to 1.
    pub fn sample(&self, p: f32) -> Sampled {
        let p = p.clamp(0.0, 1.0);
        let next = self.keys.partition_point(|k| k.at <= p);
        let (a, b, share) = if next == 0 {
            (&self.keys[0], &self.keys[0], 0.0)
        } else if next == self.keys.len() {
            let last = &self.keys[next - 1];
            (last, last, 0.0)
        } else {
            let (a, b) = (&self.keys[next - 1], &self.keys[next]);
            (a, b, b.ease.apply((p - a.at) / (b.at - a.at)))
        };
        let mix = |x: f32, y: f32| x + (y - x) * share;
        let mut angles = [[0.0; 3]; COUNT];
        for (j, out) in angles.iter_mut().enumerate() {
            for (k, v) in out.iter_mut().enumerate() {
                *v = mix(a.angles[j][k], b.angles[j][k]);
            }
        }
        Sampled {
            tuck: mix(a.tuck, b.tuck),
            lower: mix(a.lower, b.lower),
            angles,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn angle(s: &Sampled, name: &str) -> [f32; 3] {
        s.angles[JOINTS.iter().position(|j| j.name == name).unwrap()]
    }

    #[test]
    fn the_default_files_read() {
        let roll = PoseFile::parse(DEFAULT_ROLL).unwrap();
        assert_eq!(roll.turns, 1.0);
        assert_eq!(roll.sample(0.0).tuck, 0.0);
        assert_eq!(roll.sample(0.5).tuck, 1.0);
        assert_eq!(roll.sample(1.0).tuck, 0.0);
        assert!(angle(&roll.sample(0.5), "lshin_g")[0] > 90.0);
        let landing = PoseFile::parse(DEFAULT_LANDING).unwrap();
        assert_eq!(landing.turns, 0.0);
        assert!(landing.sample(0.0).lower > 0.3);
        assert_eq!(landing.sample(1.0).lower, 0.0);
        let jump = PoseFile::parse(DEFAULT_JUMP).unwrap();
        assert_eq!(jump.turns, 0.0);
        assert_eq!(jump.sample(0.5).tuck, 1.0);
        assert!(angle(&jump.sample(0.5), "lshin_g")[0] > 45.0);
        // Coming down it is no stiff drop: knees bent to take the floor, the body leaning on, and the arms forward.
        for p in [0.75, 0.85, 0.93, 1.0] {
            let s = jump.sample(p);
            for knee in ["lshin_g", "rshin_g"] {
                assert!(angle(&s, knee)[0] >= 40.0, "{knee} at {p}");
            }
            assert!(angle(&s, "torso_g")[0] >= 14.0, "torso at {p}");
            assert!(angle(&s, "rbicep_g")[0] >= 25.0, "arm at {p}");
        }
        assert_eq!(
            jump.sample(0.0).tuck,
            0.0,
            "it comes out of what the body was doing"
        );
    }

    #[test]
    fn each_default_has_a_new_version_and_differs_from_what_was_shipped() {
        for (now, old) in [
            (DEFAULT_ROLL, &SHIPPED_ROLLS[..]),
            (DEFAULT_LANDING, &SHIPPED_LANDINGS[..]),
            (DEFAULT_JUMP, &SHIPPED_JUMPS[..]),
        ] {
            let version = PoseFile::parse(now).unwrap().version;
            assert!(version >= 2);
            for (n, shipped) in old.iter().enumerate() {
                assert!(!same_text(now, shipped), "version {version} is shipped {n}");
                assert!(PoseFile::parse(shipped).unwrap().version < version);
            }
        }
    }

    #[test]
    fn left_and_right_of_a_default_are_alike() {
        // Not the jump, which is a stride.
        for text in [DEFAULT_ROLL, DEFAULT_LANDING] {
            let f = PoseFile::parse(text).unwrap();
            let s = f.sample(0.5);
            for (l, r) in JOINTS
                .iter()
                .filter(|j| j.side == Side::Left)
                .filter_map(|j| {
                    let right = format!("r{}", &j.name[1..]);
                    JOINTS
                        .iter()
                        .find(|o| o.name == right)
                        .map(|o| (j.name, o.name))
                })
            {
                assert_eq!(angle(&s, l), angle(&s, r), "{l} and {r}");
            }
        }
    }

    #[test]
    fn keys_ease_and_mix() {
        let f = PoseFile::parse(
            "[key]\nat = 0\nlthigh_g = 0\n[key]\nat = 1\nease = linear\nlthigh_g = 100 10 -20\n",
        )
        .unwrap();
        assert_eq!(angle(&f.sample(0.25), "lthigh_g"), [25.0, 2.5, -5.0]);
        let f = PoseFile::parse("[key]\nat = 0\n[key]\nat = 1\ntuck = 1\n").unwrap();
        assert_eq!(f.sample(0.5).tuck, 0.5);
        assert!(f.sample(0.25).tuck < 0.25, "smooth is the default");
    }

    #[test]
    fn before_the_first_key_and_after_the_last_it_holds() {
        let f =
            PoseFile::parse("[key]\nat = 0.4\ntuck = 0.5\n[key]\nat = 0.6\ntuck = 1\n").unwrap();
        assert_eq!(f.sample(0.1).tuck, 0.5);
        assert_eq!(f.sample(0.9).tuck, 1.0);
    }

    #[test]
    fn a_mistake_is_named_with_its_line() {
        for (text, says) in [
            ("[key]\nat = 0\nlthigh_g = lots\n", "line 3"),
            ("[key]\nat = 0\nlthigh_g = 500\n", "line 3"),
            ("[key]\nat = 2\n", "line 2"),
            ("[key]\nat = 0\nknee = 10\n", "no joint \"knee\""),
            ("[key]\nat = 0\nease = bouncy\n", "line 3"),
            ("[key]\ntuck = 1\n", "no \"at\""),
            ("[key]\nat = 0.5\n[key]\nat = 0.5\n", "go up"),
            ("turns = 0.5\n[key]\nat = 0\n", "whole number"),
            ("[key]\nat = 0\nlthigh_g = 1 2 3 4\n", "one to three"),
            ("[key]\nat = NaN\n", "line 2"),
            ("just words\n", "line 1"),
            ("# nothing\n", "no [key]"),
        ] {
            let e = PoseFile::parse(text).unwrap_err();
            assert!(e.contains(says), "{text:?} said {e}");
        }
    }

    #[test]
    fn notepad_s_mark_and_line_ends_are_fine() {
        let f = PoseFile::parse("\u{feff}turns = 1\r\n[key]\r\nat = 0 # start\r\n").unwrap();
        assert_eq!(f.turns, 1.0);
    }
}
