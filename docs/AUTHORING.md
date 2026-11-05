# Module authoring sketch (new lore for the original game and for kotor-rs)

Status: design only. The writers it relies on are in `kotor-archive` (archives), `kotor-gff`, `kotor-tables`.
The ERF/MOD/RIM, TLK and 2DA writers are type-checked and clippy-clean but their tests have not run yet
(no linker on this PC at the time of writing; see the git log for the day they went green).

## What a new area needs

A module is one `.mod` archive placed in the game's `modules` folder (or `Override` for single files).
Facts below come from what the base game's own modules contain (see `kotor-tool module <name>`).

| Resource | Type | Role | Authoring route |
|---|---|---|---|
| `module.ifo` | IFO (GFF) | entry area, start position, module scripts | copy from a base module, edit fields |
| `<area>.are` | ARE (GFF) | area name, lighting, fog, room list | copy and edit |
| `<area>.git` | GIT (GFF) | placed creatures, doors, placeables, triggers, waypoints | generated from a scene description |
| `<area>.lyt`, `.vis` | text | where each room model sits and what is visible | generated |
| room models `.mdl/.mdx`, `.wok` | binary | geometry and walkmesh | **reuse base-game rooms first**; new geometry needs an MDL writer (not started) |
| `*.utc .utp .utd .uti .utt .utw .uts .ute` | GFF templates | creatures, placeables, doors, items, triggers, waypoints, sounds, encounters | copy and edit |
| `*.dlg` | DLG (GFF) | conversations | built from a text script (see below) |
| `*.ncs` | compiled script | module and creature scripts | needs an NSS compiler; none in this repo yet |
| strings | TLK | text for new lore | custom `.tlk` via `Tlk::push`, or inline `LocString` text in the GFFs |
| `.2da` | 2DA | new items, appearances, spells | `TwoDa::push_row` into a copy of the base table |

## Proposed authoring flow

1. A small text or TOML scene file names: module, area, rooms (existing room model names and offsets), objects, dialogues.
2. `kotor-tool author build scene.toml out.mod` (to be written) does:
   - copies template GFFs from the install and edits fields in memory with `kotor-gff`;
   - writes LYT/VIS text;
   - writes DLG GFFs from a plain-text conversation script;
   - writes everything with `write_archive(ErfKind::Mod, ...)`.
3. The `.mod` is written to a folder the owner chooses. **Nothing is written to the game folder by the tool**; copying it
   into `modules` is the owner's step, and any change to his game needs his OK.
4. The same module loads in kotor-rs through `Install::load_module`, so the content survives the move to the new engine.

## Hard parts (honest)

- **Scripts.** Dialogue and quests that need conditions or rewards need NSS compilation. Options: a small compiler in this repo
  (large), or an existing tool called as an external program (needs the owner's approval to obtain). Dialogue without scripts works.
- **New geometry.** A model writer is a separate large task; reusing existing rooms is the fast path.
- **Journal, plot flags, global variables.** Quests use `global.jrl` and global vars; authoring must merge into those
  rather than replace them.
- **Voice over and lip sync.** Out of scope; text-only lines work.
- **Testing in the original game.** Needs a play session on the owner's install; I cannot do that headlessly here.

## First concrete step once the linker is back

Round-trip tests for the writers on real files, then a `kotor-tool author` command that clones an existing module under a new
name (rename IFO/ARE/GIT references) as the smallest end-to-end proof that a hand-built `.mod` is accepted.
