# Formats verified against the installed game (GOG v1.03)

"Verified" means a test in `crates/*/tests/real.rs` parses every file of that type in the install and
asserts the invariants listed. Unit tests use small synthetic inputs and truncation fuzzing.

All integers are little-endian except NCS operands (big-endian). Offsets are from file start unless noted.

## Resource type ids (KotOR 1 numbering, found by reading `chitin.key`)
Not the NWN numbering. Examples: MDL 2002, NSS 2009, NCS 2010, MOD 2011, ARE 2012, IFO 2014, WOK 2016,
2DA 2017, TLK 2018, TXI 2022, GIT 2023, DLG 2029, UTC 2027, UTI 2025, GUI 2047, LYT 3000, VIS 3001,
TPC 3007, MDX 3008. Full table: `crates/kotor-core/src/restype.rs`. Ids whose name I could not confirm
from the data (e.g. 2031) are left out.

## KEY / BIF (`kotor-archive`) - verified
- KEY: `"KEY V1  "`, bif count, key count, file-table offset, key-table offset, build year/day, 32 reserved. 64-byte header.
  File entry 12 bytes (size, name offset, name length u16, drives u16). Key entry 22 bytes: resref[16], type u16, id u32.
  `id >> 20` = BIF index, `id & 0xFFFFF` = resource index.
- BIF: `"BIFF V1  "`, variable count, fixed count (0), table offset. Entry 16 bytes: id, offset, size, type u32.
  Check: entry `i` has id index `i`. 26 BIFs. All key entries resolve; the 2da, templates, models, textures, scripts BIFs read.

## ERF / MOD / SAV / RIM (`kotor-archive`) - verified on patch.erf, modules/*.rim, lips/*.mod, TexturePacks/*.erf
- ERF family: tag (`ERF `, `MOD `, `SAV `, `HAK `) + `V1.0`, 160-byte header; language count, localized size, entry count,
  localized offset, key offset, resource offset, build year/day, description strref. Key entry 24 bytes
  (resref[16], id, type u16, unused u16); resource entry 8 bytes (offset, size).
- RIM: `"RIM V1.0"`, reserved, count, table offset. Entry 32 bytes: resref[16], type u32, id, offset, size.
- Modules ship as `<name>.rim` + `<name>_s.rim` (or a `.mod` that replaces both). `Install::load_module` layers them.
- Test: every entry's range lies inside the file (end_m01aa.rim); patch.erf and a lips `.mod` open.
- Not done: writing ERF/RIM, localized-string blocks in ERF, SAV contents.

## GFF V3.2 (`kotor-gff`) - verified, read and write
- Header 56 bytes: tag, `V3.2`, then offset+count pairs for structs, fields, labels, field data, field indices, list indices.
- Struct 12 bytes (type id, data/offset, field count; one field = direct field index, more = byte offset into field indices).
  Field 12 bytes (type, label index, inline value or offset). Label 16 bytes.
- Types 0-5 and 8 inline; 6, 7, 9, 10 (string), 11 (resref, 1-byte length), 12 (locstring), 13 (void), 16 (quaternion w,x,y,z),
  17 (vector) in the field-data block; 14 struct and 15 list by index. Type 18 never appears in the install.
- Test: all 1,000+ UTC/UTI/UTP/UTD/UTT/UTE/UTW/DLG/ITP/GUI/JRL/BIC files in the base BIFs, plus a module's IFO/ARE/GIT,
  parse, write, re-parse and compare equal. The writer reproduces content, not the original byte layout.
- Strings are decoded as Latin-1.

## 2DA (`kotor-tables`) - verified
- Binary `"2DA V2.b\n"`: tab-terminated column names ending in NUL, u32 row count, tab-terminated row labels,
  `rows*cols` u16 cell offsets, u16 data size, then NUL-terminated cell strings (offsets relative to the data start).
- Text `V2.0` is parsed too (unverified: no sample in the install).
- Test: every 2DA in the BIFs parses and is rectangular; `classes.2da` row 0 is Soldier with hit die 10.

## TLK (`kotor-tables`) - verified
- `"TLK V3.0"`, language id, string count, text offset (20-byte header). Entry 40 bytes: flags, sound resref[16],
  volume variance, pitch variance, text offset (relative to text block), text size, sound length f32. Flag bit 0 = has text.
- `dialog.tlk`: over 40k strings, over 30k with text, all decode (Windows-1252).

## TGA (`kotor-image`) - verified
- Types 2, 3, 10 and 11 at 8/24/32 bpp, either origin. All 5,601 TGAs in the BIFs decode (2x2 up to 512x512, mostly lightmaps).

## TPC (`kotor-image`) - verified (decode of mip 0; mip chain only walked)
- 128-byte header: data size (0 = uncompressed), alpha-test f32, width u16, height u16, encoding u8 (1 gray, 2 RGB, 4 RGBA),
  mip count u8, 114 pad. Non-zero data size + encoding 2 = DXT1, 4 = DXT5. Height = 6 x width marks a cube map (189 found).
- Test: all 11,452 TPCs in the four texture packs decode (5,229 DXT1, 4,509 DXT5, 1,495 RGBA, 207 RGB, 12 gray).
- Looked at one decoded GUI texture as PNG: upright, colours correct.
- Trailing TXI text is read after the mip chain; not asserted.

## MDL / MDX (`kotor-model`) - verified structurally
- MDL file: u32 0, MDL size, MDX size; everything after byte 12 uses offsets relative to byte 12.
- Geometry header 80 bytes: name[32] at 8, root node offset at 40, node count at 44. Model header follows:
  classification u8 at 80, animation array (offset, count) at 88, bbox at 104, radius, scale, supermodel name[32] at 136,
  name-table offset and count at 184/188.
- Node header 80 bytes: type flags u16, name index u16 at 4, position at 16, quaternion w,x,y,z at 28, child array at 44,
  controller keys at 56, controller data at 68.
- Mesh header (node-relative): faces array at 88 (32-byte faces: normal, plane distance, material, 3 adjacency u16, 3 index u16),
  diffuse at 152, ambient at 164, texture[32] at 168, lightmap[32] at 200, MDX stride at 332, MDX flags at 336, vertex /
  normal / colour / uv1 / uv2 offsets inside a vertex row at 340..364, vertex count u16 at 384, flags at 388..393,
  MDX data start at 404. Skin extras: bone weight offset at 424, bone index offset at 428, bone map offset/count at 432/436.
- Controllers: key 16 bytes (type, pad, rows, time start, data start, columns, pad). Orientation with 2 columns is a
  single packed word per key. The packed decoder is written from memory and is **not checked against animation playback**.
- Test: all 2,832 models parse (76,782 meshes, 968 skins, 11.6M vertices, 6,213 animations); face indices are below the vertex
  count, positions are finite, and the node count equals the header count for every model without a supermodel.
- Visual check: `kotor-tool render p_bastilabb` shows an assembled, upright humanoid body.
- Light header (92 bytes after the node header): flare radius f32 at +80, four flare arrays at +96..+144 (sizes, positions,
  colour shifts, texture-name offsets), then priority (signed i32; stock range -7..5), ambient-only, dynamic type, affect-dynamic,
  shadow, flare, fading as u32. Checked: 5,563 lights, sane value ranges. **Partly unverified:** no stock light sets the flare flag,
  so the four flare arrays have never held data here and their layout is from memory.
- Emitter header (K1): dead space, blast radius, blast length, branch count, smoothing, grid x/y, spawn type, then the strings
  update[32], render[32], blend[32], texture[32], chunk[32], two-sided, loop, render order u16, frame blending u8, depth texture[16].
  Checked on 8,870 emitters: update is one of Explosion, Fountain, Lightning, Single; render one of 7 known names (Normal,
  Linked, Billboard_to_Local_Z, ...); blend one of Normal, Lighten, Punch-Through (and 2 spelled PunchThrough). A wrong offset
  would have produced junk strings, so the layout up to the depth texture is solid. Emitter *controllers* are generic keys only.
- Not done: camera, reference, dangly, saber and AABB-node extra headers (skipped, children still walk); vertex colours; second
  index arrays; texture-animation fields; animation playback; supermodel resolution.

## WOK / DWK / PWK = BWM V1.0 (`kotor-walkmesh`) - verified
- Header 136 bytes: tag, version, kind u32, 4 use-position vectors, position, then counts and offsets for vertices, face indices,
  materials, normals, plane distances, AABB tree (44-byte nodes), adjacency (12 bytes per walkable face), edges (8 bytes), perimeters.
- Walkable surface materials are the rows with `walk = 1` in `surfacemat.2da` (1, 3-6, 9-14, 18, 30). A search over all 1,202
  area walkmeshes found that exactly the set {1,3,4,5,6,9,10,11,13} (the ids present) makes walkable-face count equal the
  adjacency count in every one. In area walkmeshes walkable faces come first. Door and placeable meshes have no adjacency table.
- Area walkmesh vertices are in world space (not room-local): at the Endar Spire entry point (15.42, 20.12) room m01aa_01a gives z = -1.275, the IFO entry height, with no layout offset applied. Door and placeable meshes are local.
- A point-in-triangle height query returns the face height at a face centroid for the Endar Spire rooms.
- AABB tree: nodes are 44 bytes (min, max, face index or -1, unknown, plane, left, right); children are node *indices*, and the
  tree has one leaf per face (all faces, not only walkable ones; 189,558 leaves over 1,202 meshes). 21 stock meshes carry boxes that
  are stale by up to 0.7 m; they are recomputed from the triangles on load (`Bwm::tree_refit` tells you). After that every tree is
  well formed.
- Queries (`floor_at`, `raycast`, `nearest_walkable`): checked against brute force on every area walkmesh, 19,376 floor and ray
  samples and 4,844 nearest-point samples, 0 mismatches. Not tested on door/placeable meshes.
- Not done: edge transitions, door-hook positions, pathfinding across rooms.

## LYT / VIS (`kotor-walkmesh`) - verified on m01aa (Endar Spire)
- Text. LYT lists `roomcount` rooms (name x y z); other sections are only counted. VIS lists each room and the rooms visible from it.
- m01aa: 15 rooms; each room's WOK exists, is an area walkmesh, and (for at least 3) has walkable floor.

## NCS (`kotor-script`) - verified (decode only)
- `"NCS V1.0"`, byte `0x42`, big-endian total size, then instructions: opcode u8, type qualifier u8, operands big-endian.
- Opcodes 0x01-0x2D decoded. Branch offsets are relative to the instruction start; `STORESTATE` has two u32 operands;
  `EQUAL/NEQUAL` with qualifier 0x24 carry a u16 struct size.
- Test: all 1,784 base-game scripts decode with instructions tiling the file exactly, every branch target on an instruction
  boundary, highest ACTION routine id 761. That confirms operand sizes. It does not confirm what each opcode does.
- Not done: the VM, the engine-routine table (about 760 routines), vector and struct semantics.
