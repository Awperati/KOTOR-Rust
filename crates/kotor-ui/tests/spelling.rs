//! Nothing shown is left in American spelling: the talk table, and what this game's own code says on screen.

use kotor_ui::shown;

/// American spellings of words the owner's list and the game's text use, in lower case.
const AMERICAN: [&str; 63] = [
    "aging",
    "civilizations",
    "judgment",
    "organizations",
    "pretenses",
    "regionalized",
    "aluminum",
    "analyze",
    "armor",
    "armored",
    "armors",
    "behavior",
    "canceled",
    "caliber",
    "center",
    "centered",
    "centers",
    "color",
    "colored",
    "colors",
    "defense",
    "defenses",
    "dialog",
    "favor",
    "favorite",
    "fiber",
    "gray",
    "harbor",
    "honor",
    "honorable",
    "humor",
    "jewelry",
    "kilometer",
    "labeled",
    "labor",
    "leveled",
    "license",
    "liter",
    "maneuver",
    "meters",
    "modeled",
    "mold",
    "neighbor",
    "offense",
    "organize",
    "paralyze",
    "plow",
    "realize",
    "recognize",
    "rumor",
    "signaled",
    "skeptical",
    "specialize",
    "sulfur",
    "theater",
    "traveled",
    "traveler",
    "valor",
    "vapor",
    "vigor",
    "woolen",
    "unarmored",
    "fulfill",
];

/// The American words in some text, as shown.
fn left_in(text: &str) -> Vec<String> {
    shown(text)
        .split(|c: char| !c.is_ascii_alphabetic())
        .map(str::to_ascii_lowercase)
        .filter(|w| AMERICAN.contains(&w.as_str()))
        .collect()
}

#[test]
fn nothing_in_the_talk_table_is_shown_in_american() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let bytes = std::fs::read(dir.join("dialog.tlk")).unwrap();
    let table = kotor_tables::Tlk::parse(&bytes).unwrap();
    let mut found = Vec::new();
    let mut lines = 0;
    for row in 0..60_000 {
        let Some(line) = table.get(row) else {
            continue;
        };
        lines += 1;
        // Tags in the table, with underscores, are never shown as words.
        if line.contains('_') {
            continue;
        }
        for word in left_in(&line) {
            found.push(format!("{row}: {word}"));
        }
    }
    assert!(lines > 40_000, "{lines} lines");
    assert!(found.is_empty(), "{} left: {found:?}", found.len());
}

/// What is between the quotes on each line of a Rust file, for strings on one line.
fn literals(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        let mut parts = line.split('"');
        parts.next();
        while let Some(inside) = parts.next() {
            out.push(inside.to_string());
            parts.next();
        }
    }
    out
}

#[test]
fn nothing_this_game_says_on_screen_is_shown_in_american() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut seen = 0;
    let mut found = Vec::new();
    for dir in ["gpu/src", "crates/kotor-ui/src"] {
        for file in std::fs::read_dir(root.join(dir)).unwrap().flatten() {
            let path = file.path();
            if path.extension().is_none_or(|e| e != "rs")
                || path.file_name().is_some_and(|n| n == "spelling.rs")
            {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for text in literals(&source) {
                seen += 1;
                for word in left_in(&text) {
                    found.push(format!("{}: {word} in {text}", path.display()));
                }
            }
        }
    }
    assert!(seen > 500, "{seen} strings");
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn the_owners_rows_are_spelt_as_in_australia_when_they_are_drawn() {
    for (written, drawn) in [
        ("Armor", "Armour"),
        ("Color of the Center", "Colour of the Centre"),
        ("Gray Defense", "Grey Defence"),
        ("Pickup radius: 3 m", "Pickup radius: 3 m"),
        ("Attack mode: On", "Attack mode: On"),
        ("Cel shading: Subtle", "Cel shading: Subtle"),
    ] {
        assert_eq!(shown(written), drawn);
    }
}
