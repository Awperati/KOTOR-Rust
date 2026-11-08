//! The player's race, as they look: the owner, "Include the playable alien races as well". See docs/CHARGEN_RACES.md.
//!
//! A race here is cosmetic. The player stays Human for the game's scripts and conversations. Another race puts its head on
//! the player's own body, which keeps the human skeleton and armour, and the body's skin takes the head's tone.
//!
//! A look keeps the race by `RACE_KEY`, a place in `RACES`, and the head by `RACE_HEAD_KEY`, a place in that race's heads.

/// The races of the Race row, in its order. The first is the game's own.
pub const RACES: [&str; 2] = ["Human", "Twi'lek"];
pub const RACE_KEY: &str = "race";
pub const RACE_HEAD_KEY: &str = "race_head";
pub const TWILEK: usize = 1;

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
        _ => Vec::new(),
    }
}

fn every_head() -> Vec<(&'static str, &'static str)> {
    TWILEK_WOMEN
        .iter()
        .chain(TWILEK_MEN.iter())
        .copied()
        .chain(mod_twilek_women())
        .collect()
}

/// The race a look keeps, by its numbers: 0, Human, for one it does not know.
pub fn race_of(get: impl Fn(&str) -> i32) -> usize {
    let r = get(RACE_KEY);
    if (1..RACES.len() as i32).contains(&r) {
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
}
