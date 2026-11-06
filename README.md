# kotor-rs

A clean-room Rust engine for Star Wars: Knights of the Old Republic (KotOR 1). It reads the owner's installed game
files at runtime. No game assets, BioWare code or decompiled code are in this repo.

Start with `PLAN.md` (scope, decisions, risks), `docs/FORMATS.md` (what is verified) and `docs/SOURCES.md`.

## Crates

| Crate | What |
|---|---|
| `kotor-core` | errors, little-endian reader, resource types and names, install-path lookup |
| `kotor-archive` | KEY/BIF, ERF/MOD/SAV, RIM, and `Install` (layers modules and texture packs over the BIFs) |
| `kotor-gff` | GFF V3.2 read and write |
| `kotor-tables` | 2DA and TLK |
| `kotor-image` | TGA, TPC (DXT1/DXT5) and a PNG writer |
| `kotor-model` | MDL/MDX models, OBJ export |
| `kotor-walkmesh` | BWM walkmeshes, LYT and VIS layouts |
| `kotor-script` | NCS bytecode disassembler |
| `kotor-tool` | headless inspection CLI |

## Use

The game is found through `KOTOR_DIR`, else `<game folder>`. It is only ever read.

```
cargo run -p kotor-tool -- module end_m01aa
cargo run -p kotor-tool -- render p_bastilabb out/bastila.png
cargo run -p kotor-tool -- obj p_bastilabb out/bastila.obj
cargo run -p kotor-tool -- disasm k_def_attacked01
cargo run -p kotor-tool -- 2da appearance 5
```

## Checks

```
powershell -File scripts/check.ps1     # fmt, clippy -D warnings, all tests
```

Tests that read the game skip themselves when it is missing. The full run takes about 40 seconds.

## Credits

- **Dubird**, "New undies for female Exile" (2007), the blue two-piece textures `pfbama01`, `pfbamB01`, `pfbamc01` and `pfbamd01`. They are read at run time from a folder on the player's own computer (`KOTOR_MODS_DIR`, or `Blue Undies-3-1` beside the projects), and are not part of this repository or any build. Their readme asks that they be shared only with it, and that credit be given.
