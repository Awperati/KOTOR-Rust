//! The player's race, as they look: the owner, "Include the playable alien races as well". See docs/CHARGEN_RACES.md.
//!
//! A race here is cosmetic. The player stays Human for the game's scripts and conversations. Another race puts its head on
//! the player's own body, which keeps the human skeleton and armour, and the body's skin takes the head's tone. A race with a
//! body of its own wears it whatever is on: the Jawa from its pack, which says its own lines where the pack has them, and the
//! game's Rodian, Duros, Wookiee and Trandoshan, whose presets are their rows' pictures.
//!
//! A look keeps the race by `RACE_KEY`, a place in `RACES`, and the head by `RACE_HEAD_KEY`, a place in that race's heads.

/// The races of the Race row, in its order. The first is the game's own.
pub const RACES: [&str; 7] = [
    "Human",
    "Twi'lek",
    "Jawa",
    "Rodian",
    "Duros",
    "Wookiee",
    "Trandoshan",
];
pub const RACE_KEY: &str = "race";
pub const RACE_HEAD_KEY: &str = "race_head";
pub const TWILEK: usize = 1;
/// A Jawa, from the playable Jawa mod pack when the player has it on: a body of its own, robe and hood, on the hero's
/// skeleton. The body keeps the name the pack loader gives it, as the pack's own would take the place of the first man's.
pub const JAWA: usize = 2;
pub const JAWA_BODY: &str = "p_jawab";
/// The game's own aliens, each one model that is head, body and clothes: their `appearance.2da` rows are type `F`, and they
/// play the human clips. Each wears its body in every outfit, as the game's characters of those races do.
pub const RODIAN: usize = 3;
pub const DUROS: usize = 4;
pub const WOOKIEE: usize = 5;
pub const TRANDOSHAN: usize = 6;

/// A preset of a race with a body of its own: the race, for a woman or a man only or for both, the name the look keeps, the
/// model, and the picture in place of the model's own (the `racetex` of the game's rows for that race).
struct Body {
    race: usize,
    woman: Option<bool>,
    key: &'static str,
    model: &'static str,
    texture: Option<&'static str>,
}

const fn body(
    race: usize,
    woman: Option<bool>,
    key: &'static str,
    model: &'static str,
    texture: Option<&'static str>,
) -> Body {
    Body {
        race,
        woman,
        key,
        model,
        texture,
    }
}

/// Every preset of the races with a body of their own, in each race's order. The Jawa's hood is its head, so its key names no
/// model.
const BODIES: &[Body] = &[
    body(JAWA, None, "p_jawah", JAWA_BODY, None),
    body(RODIAN, None, "n_rodian", "n_rodian", None),
    body(RODIAN, None, "n_rodian03", "n_rodian", Some("n_rodian03")),
    body(RODIAN, None, "n_rodian04", "n_rodian", Some("n_rodian04")),
    body(RODIAN, None, "n_rodian05", "n_rodian", Some("n_rodian05")),
    body(DUROS, None, "n_duros", "n_duros", None),
    body(DUROS, None, "n_duros02", "n_duros", Some("n_duros02")),
    body(DUROS, None, "n_duros03", "n_duros", Some("n_duros03")),
    body(WOOKIEE, Some(false), "n_wookiem", "n_wookiem", None),
    body(
        WOOKIEE,
        Some(false),
        "n_wookiem02",
        "n_wookiem",
        Some("n_wookiem02"),
    ),
    body(
        WOOKIEE,
        Some(false),
        "n_wookiem03",
        "n_wookiem",
        Some("n_wookiem03"),
    ),
    body(
        WOOKIEE,
        Some(false),
        "n_wookiem04",
        "n_wookiem",
        Some("n_wookiem04"),
    ),
    body(
        WOOKIEE,
        Some(false),
        "n_wookiem05",
        "n_wookiem",
        Some("n_wookiem05"),
    ),
    body(WOOKIEE, Some(true), "n_wookief", "n_wookief", None),
    body(
        WOOKIEE,
        Some(true),
        "n_wookief02",
        "n_wookief",
        Some("n_wookief02"),
    ),
    body(
        WOOKIEE,
        Some(true),
        "n_wookief03",
        "n_wookief",
        Some("n_wookief03"),
    ),
    body(
        WOOKIEE,
        Some(true),
        "n_wookief04",
        "n_wookief",
        Some("n_wookief04"),
    ),
    body(
        WOOKIEE,
        Some(true),
        "n_wookief05",
        "n_wookief",
        Some("n_wookief05"),
    ),
    body(TRANDOSHAN, None, "n_trandoshan", "n_trandoshan", None),
    body(
        TRANDOSHAN,
        None,
        "p_trndoshan02",
        "n_trandoshan",
        Some("p_trndoshan02"),
    ),
];
/// The start of the Jawa's voice lines, in place of a hero's `p_plyer..` set.
const JAWA_VOICE: &str = "p_jawamw_";

/// Whether the Jawa's pack is on.
static JAWA_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the mod packs bring the Jawa: the Race row offers it only then.
pub fn set_mod_jawa(on: bool) {
    JAWA_ON.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the Race row offers a race now: the game's own always, a mod pack's when it is on.
pub fn offered(race: usize) -> bool {
    match race {
        JAWA => JAWA_ON.load(std::sync::atomic::Ordering::Relaxed),
        r => r < RACES.len(),
    }
}

/// Whether a race wears a body of its own in place of the player's, whatever they have on.
pub fn own_body(race: usize) -> bool {
    BODIES.iter().any(|b| b.race == race)
}

/// The body that goes with a race's preset, for a race with a body of its own: its model, and the picture in place of the
/// model's own.
pub fn body_of_head(head: &str) -> Option<(&'static str, Option<&'static str>)> {
    BODIES
        .iter()
        .find(|b| b.key.eq_ignore_ascii_case(head))
        .map(|b| (b.model, b.texture))
}

/// The game's own Jawa, whose eyes glow in its hood: the pack's body has the hood and not the eyes.
const JAWA_EYES_FROM: &str = "c_jawa";
const EYES: &str = "eyes";
/// The Jawa's robe, hood and all, in both models.
const HOOD: &str = "jawa_torso";
const NECK: &str = "neck_g";

/// A Jawa body with the eyes of the game's own Jawa in its hood. Its hood is the game's, made larger and set a little
/// differently, so the eyes go to the same place in the hood's box, and move with the hood as its nearest point does. A
/// body with eyes already, or no hood, is left as it is.
pub fn add_jawa_eyes(
    inst: &mut kotor_archive::Install,
    body: &mut kotor_model::Model,
) -> Option<()> {
    use kotor_core::ResType;
    if body.root.find(EYES).is_some() {
        return None;
    }
    let mdl = inst.read(JAWA_EYES_FROM, ResType::MDL).ok()?;
    let mdx = inst.read(JAWA_EYES_FROM, ResType::MDX).ok()?;
    let theirs = kotor_model::parse_model(&mdl, &mdx).ok()?;
    let (eye_points, eyes) = rest_points(&theirs.root, EYES)?;
    let their_hood = hood_points(&theirs.root)?;
    let our_hood = hood_points(&body.root)?;
    let (from, to) = (bounds(&their_hood.0), bounds(&our_hood.0));
    let fit = |v: [f32; 3]| {
        [0, 1, 2].map(|i| {
            let span = (from.1[i] - from.0[i]).max(1e-6);
            to.0[i] + (v[i] - from.0[i]) * (to.1[i] - to.0[i]) / span
        })
    };
    let mut positions: Vec<[f32; 3]> = eye_points.iter().map(|v| fit(*v)).collect();
    // The pack's hood leans further forward than the game's, and its chin would hide eyes put by the box alone: they go up
    // to the middle of the opening.
    if let Some(middle) = opening_middle(&our_hood.0, to) {
        let (lo, hi) = bounds(&positions);
        let up = middle - (lo[2] + hi[2]) / 2.0;
        for p in &mut positions {
            p[2] += up;
        }
    }
    let hood_mesh = body.root.find(HOOD)?.mesh.as_ref()?;
    let hood_skin = hood_mesh.skin.as_ref()?;
    // Each point of the eyes moves as the hood's nearest point does.
    let nearest = |p: [f32; 3]| {
        our_hood
            .0
            .iter()
            .zip(&our_hood.1)
            .min_by(|a, b| dist2(*a.0, p).total_cmp(&dist2(*b.0, p)))
            .map_or(0, |(_, i)| *i)
    };
    let picks: Vec<usize> = positions.iter().map(|p| nearest(*p)).collect();
    let mut mesh = eyes.mesh.clone()?;
    mesh.skin = Some(kotor_model::Skin {
        weights: picks.iter().map(|i| hood_skin.weights[*i]).collect(),
        bones: picks.iter().map(|i| hood_skin.bones[*i]).collect(),
        bone_map: hood_skin.bone_map.clone(),
    });
    mesh.positions = positions;
    let mut next = 0u16;
    body.root.walk(&mut |n| next = next.max(n.index + 1));
    body.root.children.push(kotor_model::Node {
        name: EYES.to_string(),
        index: next,
        number: next,
        flags: kotor_model::NODE_HEADER | kotor_model::NODE_MESH | kotor_model::NODE_SKIN,
        position: [0.0; 3],
        orientation: [1.0, 0.0, 0.0, 0.0],
        controllers: Vec::new(),
        mesh: Some(mesh),
        light: None,
        emitter: None,
        children: Vec::new(),
    });
    Some(())
}

/// The height of the middle of a hood's opening: on the front half of its middle line, the widest gap between its points from
/// the chin to the brow.
fn opening_middle(hood: &[[f32; 3]], (lo, hi): ([f32; 3], [f32; 3])) -> Option<f32> {
    let centre = (lo[0] + hi[0]) / 2.0;
    let front = (lo[1] + hi[1]) / 2.0;
    let mut heights: Vec<f32> = hood
        .iter()
        .filter(|p| (p[0] - centre).abs() < 0.03 && p[1] > front)
        .map(|p| p[2])
        .collect();
    heights.sort_by(f32::total_cmp);
    heights
        .windows(2)
        .max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0])))
        .map(|w| (w[0] + w[1]) / 2.0)
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}

fn bounds(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in points {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    (lo, hi)
}

/// The robe's points above the neck, the hood, in the model at rest, with their places in the robe's mesh.
fn hood_points(root: &kotor_model::Node) -> Option<(Vec<[f32; 3]>, Vec<usize>)> {
    let (_, neck) = rest_place(root, NECK)?;
    let (points, _) = rest_points(root, HOOD)?;
    Some(
        points
            .into_iter()
            .enumerate()
            .filter(|(_, p)| p[2] > neck[2])
            .map(|(i, p)| (p, i))
            .unzip(),
    )
}

/// A mesh's points in its model at rest, and its node.
fn rest_points<'a>(
    root: &'a kotor_model::Node,
    name: &str,
) -> Option<(Vec<[f32; 3]>, &'a kotor_model::Node)> {
    let (turned, at) = rest_place(root, name)?;
    let node = root.find(name)?;
    let points = node
        .mesh
        .as_ref()?
        .positions
        .iter()
        .map(|v| {
            let v = turn(turned, *v);
            [at[0] + v[0], at[1] + v[1], at[2] + v[2]]
        })
        .collect();
    Some((points, node))
}

/// A node's turn and place in its model at rest.
fn rest_place(root: &kotor_model::Node, name: &str) -> Option<([f32; 4], [f32; 3])> {
    fn go(
        n: &kotor_model::Node,
        q: [f32; 4],
        at: [f32; 3],
        name: &str,
    ) -> Option<([f32; 4], [f32; 3])> {
        let p = turn(q, n.position);
        let at = [at[0] + p[0], at[1] + p[1], at[2] + p[2]];
        let q = mul(q, n.orientation);
        if n.name.eq_ignore_ascii_case(name) {
            return Some((q, at));
        }
        n.children.iter().find_map(|c| go(c, q, at, name))
    }
    go(root, [1.0, 0.0, 0.0, 0.0], [0.0; 3], name)
}

/// `v` turned by the quaternion `q`, `[w, x, y, z]`.
fn turn(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [w, x, y, z] = q;
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let t = cross([x, y, z], v).map(|c| 2.0 * c);
    let c = cross([x, y, z], t);
    [0, 1, 2].map(|i| v[i] + w * t[i] + c[i])
}

fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

/// The voice line a race says in place of a hero's `sound`, `p_plyer` then two letters for the set, for a race with a voice
/// of its own. The caller checks it is there.
pub fn voice_of(race: usize, sound: &str) -> Option<String> {
    let s = sound.to_ascii_lowercase();
    if race != JAWA || !s.starts_with("p_plyer") || s.as_bytes().get(9) != Some(&b'_') {
        return None;
    }
    Some(format!("{JAWA_VOICE}{}", &s[10..]))
}

/// The game's Twi'lek heads, from `heads.2da` rows 61 to 71: each head's model, and its texture.
const TWILEK_WOMEN: [(&str, &str); 4] = [
    ("twilek_f", "twilek_f01"),
    ("twilek_f02", "twilek_f02"),
    ("twilek_f03", "twilek_f03"),
    ("twilek_f04", "twilek_f04"),
];
const TWILEK_MEN: [(&str, &str); 3] = [
    ("twilek_m", "twilek_m01"),
    ("twilek_m02", "twilek_m02"),
    ("twilek_m03", "twilek_m03"),
];

/// Twi'lek women's heads from the player's mod packs, after the game's own: model and texture.
static MOD_TWILEK_WOMEN: std::sync::RwLock<Vec<(&'static str, &'static str)>> =
    std::sync::RwLock::new(Vec::new());

/// The heads the mod packs bring for Twi'lek women, model and texture, in place of any before. Each name is kept for the
/// whole run, as the heads are few.
pub fn set_mod_twilek_women(heads: Vec<(String, String)>) {
    let leak = |s: String| -> &'static str { Box::leak(s.to_ascii_lowercase().into_boxed_str()) };
    let mut list = MOD_TWILEK_WOMEN.write().unwrap_or_else(|e| e.into_inner());
    if list.len() == heads.len()
        && list
            .iter()
            .zip(&heads)
            .all(|(a, b)| a.0.eq_ignore_ascii_case(&b.0))
    {
        return;
    }
    *list = heads.into_iter().map(|(m, t)| (leak(m), leak(t))).collect();
}

fn mod_twilek_women() -> Vec<(&'static str, &'static str)> {
    MOD_TWILEK_WOMEN
        .read()
        .map(|l| l.clone())
        .unwrap_or_default()
}

/// A race's heads for a woman or a man: model and texture. The game's own come first, then any from the mod packs.
pub fn heads(race: usize, woman: bool) -> Vec<(&'static str, &'static str)> {
    match (race, woman) {
        (TWILEK, true) => TWILEK_WOMEN
            .iter()
            .copied()
            .chain(mod_twilek_women())
            .collect(),
        (TWILEK, false) => TWILEK_MEN.to_vec(),
        _ => BODIES
            .iter()
            .filter(|b| b.race == race && b.woman.is_none_or(|w| w == woman))
            .map(|b| (b.key, b.key))
            .collect(),
    }
}

/// The heads that go on the player's body, whose marks are painted as on a player's own.
fn every_head() -> Vec<(&'static str, &'static str)> {
    TWILEK_WOMEN
        .iter()
        .chain(TWILEK_MEN.iter())
        .copied()
        .chain(mod_twilek_women())
        .collect()
}

/// The race a look keeps, by its numbers: 0, Human, for one it does not know or one not offered now.
pub fn race_of(get: impl Fn(&str) -> i32) -> usize {
    let r = get(RACE_KEY);
    if (1..RACES.len() as i32).contains(&r) && offered(r as usize) {
        r as usize
    } else {
        0
    }
}

/// The head a look puts on the player in place of their own: model and texture. None for a Human.
pub fn head_of(get: impl Fn(&str) -> i32, woman: bool) -> Option<(&'static str, &'static str)> {
    let list = heads(race_of(&get), woman);
    if list.is_empty() {
        return None;
    }
    let n = get(RACE_HEAD_KEY).clamp(0, list.len() as i32 - 1) as usize;
    Some(list[n])
}

/// Whether a texture is a race's head: its marks are painted on it, as on a player's own.
pub fn is_race_head(texture: &str) -> bool {
    let t = texture.to_ascii_lowercase();
    every_head().iter().any(|(_, tex)| *tex == t)
}

/// The model a race head's texture belongs to.
pub fn model_of_head(texture: &str) -> Option<&'static str> {
    let t = texture.to_ascii_lowercase();
    every_head()
        .into_iter()
        .find(|(_, tex)| *tex == t)
        .map(|(model, _)| model)
}

/// Whether a texture is one of the player's bodies, bare or in armour, whose skin takes a race head's tone.
pub fn is_player_body(texture: &str) -> bool {
    let t = texture.to_ascii_lowercase();
    t.starts_with("pfb") || t.starts_with("pmb")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_look_picks_a_race_and_its_head() {
        let human = |_: &str| 0;
        assert_eq!(race_of(human), 0);
        assert_eq!(head_of(human, true), None);
        let twilek = |k: &str| match k {
            RACE_KEY => 1,
            RACE_HEAD_KEY => 9,
            _ => 0,
        };
        assert_eq!(race_of(twilek), TWILEK);
        assert_eq!(head_of(twilek, true), Some(("twilek_f04", "twilek_f04")));
        assert_eq!(head_of(twilek, false), Some(("twilek_m03", "twilek_m03")));
        assert_eq!(race_of(|_| 7), 0);
        assert!(is_race_head("Twilek_F01") && !is_race_head("pfha01"));
        assert_eq!(model_of_head("twilek_m01"), Some("twilek_m"));
        assert!(is_player_body("PFBAM01") && !is_player_body("twilek_f01"));
        // A mod pack's heads come after the game's own, for women only.
        set_mod_twilek_women(vec![("P_TwiH1".to_string(), "P_Twi1".to_string())]);
        assert_eq!(heads(TWILEK, true).len(), 5);
        assert_eq!(heads(TWILEK, true)[4], ("p_twih1", "p_twi1"));
        assert_eq!(heads(TWILEK, false).len(), 3);
        assert!(is_race_head("P_Twi1"));
        assert_eq!(model_of_head("p_twi1"), Some("p_twih1"));
        set_mod_twilek_women(Vec::new());
        assert_eq!(heads(TWILEK, true).len(), 4);
    }

    #[test]
    fn a_jawa_is_offered_with_its_pack_and_wears_its_own_body_and_voice() {
        let jawa = |k: &str| match k {
            RACE_KEY => JAWA as i32,
            _ => 0,
        };
        set_mod_jawa(false);
        assert_eq!(race_of(jawa), 0, "not without the pack");
        assert!(!offered(JAWA) && offered(TWILEK));
        set_mod_jawa(true);
        assert_eq!(race_of(jawa), JAWA);
        assert_eq!(head_of(jawa, false), Some(("p_jawah", "p_jawah")));
        assert_eq!(head_of(jawa, true), Some(("p_jawah", "p_jawah")));
        assert!(own_body(JAWA) && !own_body(TWILEK) && !own_body(0));
        assert_eq!(body_of_head("P_JawaH"), Some(("p_jawab", None)));
        assert_eq!(body_of_head("twilek_f"), None);
        assert!(!is_race_head("p_jawah"), "no marks on a hood");
        assert_eq!(
            voice_of(JAWA, "P_PLYERFB_ATK1").as_deref(),
            Some("p_jawamw_atk1")
        );
        assert_eq!(voice_of(JAWA, "cb_ht_bllrgmetal"), None);
        assert_eq!(voice_of(TWILEK, "p_plyermw_atk1"), None);
        set_mod_jawa(false);
    }

    #[test]
    fn the_games_aliens_wear_their_own_bodies_in_their_rows_pictures() {
        let race = |r: usize, head: i32| {
            move |k: &str| match k {
                RACE_KEY => r as i32,
                RACE_HEAD_KEY => head,
                _ => 0,
            }
        };
        for r in [RODIAN, DUROS, WOOKIEE, TRANDOSHAN] {
            assert!(offered(r) && own_body(r), "{}", RACES[r]);
            assert_eq!(race_of(race(r, 0)), r);
            for woman in [false, true] {
                assert!(!heads(r, woman).is_empty(), "{} {woman}", RACES[r]);
            }
        }
        assert_eq!(heads(RODIAN, false).len(), 4);
        assert_eq!(heads(DUROS, true).len(), 3);
        assert_eq!(heads(TRANDOSHAN, false).len(), 2);
        // A Wookiee man and woman are two models.
        let man = head_of(race(WOOKIEE, 2), false).unwrap().0;
        let woman = head_of(race(WOOKIEE, 2), true).unwrap().0;
        assert_eq!(body_of_head(man), Some(("n_wookiem", Some("n_wookiem03"))));
        assert_eq!(
            body_of_head(woman),
            Some(("n_wookief", Some("n_wookief03")))
        );
        // The first is the model's own picture.
        let first = head_of(race(RODIAN, 0), false).unwrap().0;
        assert_eq!(body_of_head(first), Some(("n_rodian", None)));
        assert!(
            !is_race_head("n_rodian03"),
            "no marks painted on a whole body"
        );
    }
}
