//! Australian spelling of what is shown, as the owner asked: done when text is drawn, so the game's files and what scripts
//! compare stay as they are. "Lightsaber" stays: it is a name.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// Words with `-or` that take `-our`, and all their endings. "Rancor" is a beast here, and stays.
const OUR: &[&str] = &[
    "armor",
    "behavior",
    "candor",
    "clamor",
    "color",
    "demeanor",
    "dishonor",
    "enamor",
    "endeavor",
    "favor",
    "fervor",
    "flavor",
    "glamor",
    "harbor",
    "honor",
    "humor",
    "labor",
    "multicolor",
    "neighbor",
    "odor",
    "parlor",
    "rigor",
    "rumor",
    "savior",
    "savor",
    "splendor",
    "tumor",
    "unarmor",
    "valor",
    "vapor",
    "vigor",
];
const OUR_ENDINGS: [&str; 11] = [
    "", "s", "ed", "ing", "able", "ably", "ful", "less", "ite", "ites", "y",
];
/// Words with `-ize` that take `-ise`, and all their endings, from what the talk table has. Not "prize", "size" or "seize".
const ISE: &[&str] = &[
    "agon",
    "antagon",
    "apolog",
    "author",
    "capital",
    "central",
    "character",
    "civil",
    "colon",
    "compartmental",
    "computer",
    "critic",
    "crystall",
    "custom",
    "demon",
    "depressur",
    "desensit",
    "disorgan",
    "emphas",
    "energ",
    "final",
    "fratern",
    "harmon",
    "hypnot",
    "idol",
    "immobil",
    "initial",
    "jeopard",
    "legal",
    "liquid",
    "lobotom",
    "local",
    "material",
    "maxim",
    "memor",
    "militar",
    "miniatur",
    "minim",
    "mobil",
    "modern",
    "neutral",
    "normal",
    "optim",
    "organ",
    "ostrac",
    "oxid",
    "paral",
    "patron",
    "personal",
    "pressur",
    "prior",
    "prophes",
    "pulver",
    "rational",
    "real",
    "recogn",
    "region",
    "regional",
    "reinitial",
    "social",
    "special",
    "stabil",
    "steril",
    "subsid",
    "summar",
    "symbol",
    "sympath",
    "synchron",
    "synthes",
    "terror",
    "theor",
    "unauthor",
    "uncivil",
    "util",
    "vapor",
    "victim",
    "woman",
];
const ISE_ENDINGS: [(&str, &str); 8] = [
    ("ize", "ise"),
    ("izes", "ises"),
    ("ized", "ised"),
    ("izing", "ising"),
    ("izer", "iser"),
    ("izers", "isers"),
    ("ization", "isation"),
    ("izations", "isations"),
];
/// Words with a single `l` that double it before an ending.
const DOUBLE_L: &[&str] = &[
    "cancel", "channel", "chisel", "duel", "equal", "fuel", "funnel", "jewel", "label", "level",
    "marshal", "martial", "model", "panel", "quarrel", "revel", "rival", "shovel", "signal",
    "snivel", "spiral", "total", "travel", "tunnel", "unravel",
];
const DOUBLE_L_ENDINGS: [&str; 4] = ["ed", "ing", "er", "ers"];
/// The rest, each form by itself.
const WORDS: &[(&str, &str)] = &[
    ("aging", "ageing"),
    ("analyze", "analyse"),
    ("analyzed", "analysed"),
    ("analyzes", "analyses"),
    ("analyzing", "analysing"),
    ("armory", "armoury"),
    ("artifact", "artefact"),
    ("artifacts", "artefacts"),
    ("caliber", "calibre"),
    ("catalog", "catalogue"),
    ("catalogs", "catalogues"),
    ("center", "centre"),
    ("centered", "centred"),
    ("centers", "centres"),
    ("centimeter", "centimetre"),
    ("centimeters", "centimetres"),
    ("cozy", "cosy"),
    ("councilor", "councillor"),
    ("councilors", "councillors"),
    ("counselor", "counsellor"),
    ("counselors", "counsellors"),
    ("defense", "defence"),
    ("defenseless", "defenceless"),
    ("defenses", "defences"),
    ("dialog", "dialogue"),
    ("enroll", "enrol"),
    ("fiber", "fibre"),
    ("fibers", "fibres"),
    ("fulfill", "fulfil"),
    ("gray", "grey"),
    ("grayish", "greyish"),
    ("instill", "instil"),
    ("jewelry", "jewellery"),
    ("judgment", "judgement"),
    ("judgments", "judgements"),
    ("kilometer", "kilometre"),
    ("kilometers", "kilometres"),
    ("license", "licence"),
    ("licenses", "licences"),
    ("liter", "litre"),
    ("liters", "litres"),
    ("maneuver", "manoeuvre"),
    ("maneuvered", "manoeuvred"),
    ("maneuvering", "manoeuvring"),
    ("maneuvers", "manoeuvres"),
    ("marvelous", "marvellous"),
    ("meager", "meagre"),
    ("meters", "metres"),
    ("mold", "mould"),
    ("mom", "mum"),
    ("mommy", "mummy"),
    ("mustache", "moustache"),
    ("offense", "offence"),
    ("offenses", "offences"),
    ("paralyze", "paralyse"),
    ("paralyzed", "paralysed"),
    ("paralyzation", "paralysation"),
    ("paralyzing", "paralysing"),
    ("pretense", "pretence"),
    ("pretenses", "pretences"),
    ("skeptic", "sceptic"),
    ("skeptical", "sceptical"),
    ("skepticism", "scepticism"),
    ("skillful", "skilful"),
    ("skillfully", "skilfully"),
    ("willful", "wilful"),
    ("willfully", "wilfully"),
    ("theater", "theatre"),
];

thread_local! {
    static ON: Cell<bool> = const { Cell::new(true) };
    static TABLE: RefCell<HashMap<String, String>> = RefCell::new(table());
}

fn table() -> HashMap<String, String> {
    let mut t: HashMap<String, String> = WORDS
        .iter()
        .map(|(us, au)| (us.to_string(), au.to_string()))
        .collect();
    for stem in OUR {
        let base = &stem[..stem.len() - 2];
        for ending in OUR_ENDINGS {
            t.insert(format!("{base}or{ending}"), format!("{base}our{ending}"));
        }
    }
    for stem in ISE {
        for (us, au) in ISE_ENDINGS {
            t.insert(format!("{stem}{us}"), format!("{stem}{au}"));
        }
    }
    for word in DOUBLE_L {
        for ending in DOUBLE_L_ENDINGS {
            t.insert(format!("{word}{ending}"), format!("{word}l{ending}"));
        }
    }
    t
}

/// The setting: Australian, or as the game has it. Returns whether that is a change.
pub fn set_australian(on: bool) -> bool {
    ON.with(|o| o.replace(on)) != on
}

/// A word in the case it came in.
fn in_case_of(word: &str, new: &str) -> String {
    let mut letters = word.chars();
    let first_up = letters.next().is_some_and(char::is_uppercase);
    if first_up && word.len() > 1 && letters.all(char::is_uppercase) {
        return new.to_uppercase();
    }
    if first_up {
        let mut out = new.to_string();
        out[..1].make_ascii_uppercase();
        return out;
    }
    new.to_string()
}

/// What to show for some text.
pub fn shown(text: &str) -> Cow<'_, str> {
    if !ON.with(Cell::get) {
        return Cow::Borrowed(text);
    }
    TABLE.with_borrow(|table| {
        let mut out: Option<String> = None;
        let mut done = 0;
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if !bytes[i].is_ascii_alphabetic() {
                i += 1;
                continue;
            }
            let from = i;
            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            let word = &text[from..i];
            let lower = word.to_ascii_lowercase();
            // A meter that reads a dial stays. One that follows a number is a distance.
            let before = text[..from].trim_end_matches(['-', ' ']);
            let distance = lower == "meter" && before.ends_with(|c: char| c.is_ascii_digit());
            let new = if distance {
                Some("metre")
            } else {
                table.get(&lower).map(String::as_str)
            };
            if let Some(new) = new {
                let o = out.get_or_insert_with(String::new);
                o.push_str(&text[done..from]);
                o.push_str(&in_case_of(word, new));
                done = i;
            }
        }
        match out {
            Some(mut o) => {
                o.push_str(&text[done..]);
                Cow::Owned(o)
            }
            None => Cow::Borrowed(text),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_shown_is_spelt_as_in_australia_and_keeps_its_case() {
        for (us, au) in [
            ("Restricted By Armor", "Restricted By Armour"),
            ("ARMOR and color", "ARMOUR and colour"),
            ("Defense: the Center's gray", "Defence: the Centre's grey"),
            (
                "a 10-meter radius, 3 meter, 4 meters",
                "a 10-metre radius, 3 metre, 4 metres",
            ),
            ("a power meter", "a power meter"),
            ("realized, organization", "realised, organisation"),
            ("He traveled, canceling", "He travelled, cancelling"),
            (
                "Knight Valor is your favorite",
                "Knight Valour is your favourite",
            ),
            ("MANEUVERS", "MANOEUVRES"),
        ] {
            assert_eq!(shown(us), au);
        }
    }

    #[test]
    fn names_and_other_words_stay_and_the_setting_turns_it_off() {
        for same in [
            "Lightsaber",
            "A rancor, a prize, the size",
            "Doctor and actor, a door, the floor, error, Tutor",
            "<FullName> visits the Sith.",
            "Motor, mirror, terror, horror, sensor, senator",
        ] {
            assert!(matches!(shown(same), Cow::Borrowed(_)), "{same}");
        }
        set_australian(false);
        assert_eq!(shown("Armor"), "Armor");
        set_australian(true);
    }
}
