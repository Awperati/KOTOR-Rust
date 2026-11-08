# Voice lines named but missing from the install (scan of 8 Nov)

Run with `cargo run --release -p kotor-game --example voice_scan`. Of 21,207 dialogue lines: 14,142 have their voice file,
**6,149 name a voice file the install does not have**, 915 have no voice at all (written to be read), 1 line has a sound in the
talk table with no file. The missing ones are the same files on every machine with this install: the files are not in the
archives, so no code fix brings them back. They play silent with their text.

| Place (dialogue file prefix) | Lines with a missing voice |
| --- | --- |
| Manaan (`man`) | 1340 |
| Tatooine (`tat`) | 1114 |
| Kashyyyk (`kas`) | 945 |
| Taris (`tar`) | 741 |
| Dantooine (`dan`) | 477 |
| Unknown World (`unk`) | 372 |
| Ebon Hawk (`ebo`) | 349 |
| Korriban (`kor`) | 327 |
| Leviathan (`lev`) | 105 |
| Party members (`k_h`, `k_s`, `k_p`, `k_3x`) | about 150 |
| Endar Spire (`end`) | 65 |
| Others | about 70 |

**Assumption:** the prefix names the place (`man` Manaan and so on). The list per speaker, with an example file name for each,
is in the scan output (`voice-scan-8nov.txt` in the notes folder). Mod packs that bring voice files (checked in `Mods`) are read
first, so adding the files there fills the gaps.
