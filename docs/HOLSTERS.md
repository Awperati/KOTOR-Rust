# Holsters: where a weapon is when it is not in the hand, and how it gets there

The owner, 8 Nov: right-handed blasters on the left hip, left-handed blasters on the right hip, two-handed blasters on the back.
Right-handed blades on the back, diagonally from the right shoulder to the left hip, left-handed blades the same from the left
shoulder to the right hip. The characters draw and holster, the blasters too. On entering and leaving combat mode they draw and
holster. The owner's standing order of 8 Nov: take the recommended option and write down what was assumed. This is that.

## What the files give

- A weapon's class is its `baseitems.2da` row's `weaponwield`: 1 stun baton, 2 one-handed blade (sword, vibroblade, lightsaber,
  short lightsaber), 3 two-handed melee (staff, double blade, double saber), 4 pistol, 5 rifle (also carbine, bowcaster), 6 heavy
  repeater. Which hand: the equipped slot, `RIGHT_HAND_SLOT` or `LEFT_HAND_SLOT` (`equipped` of the creature).
- A body's nodes: `rhand` and `lhand` (where a weapon is held today, `Stage::hands`), and for the places on the body `torso_g`,
  `torsoUpr_g`, `pelvis_g`, `rthigh_g`, `lthigh_g`, `neck_g` (read from `pmbam.mdl` and `S_Male02`). A place on the body is a node and
  an offset in that node's frame, so it follows every clip.
- **No holster or sheath model is in the game files.** I searched the key file: only `cs_swordsheath`, a prop of one cutscene.
  Holsters are therefore plain shapes made by this game (a small box and strap for a pistol, a bar for a sling, a tube for a blade
  sheath), built in code and drawn in dull leather and gun-metal. A hand-made model of the owner's, when there is one, goes in
  the owner's meshes folder and takes their place (`docs/BLENDER_MESHES.md`).
- The clips: `equip` (1.0 s, the hand moves to the body and up), the ready stances `g1r1`, `g5r1`, `g6r1`, `g8r1`, `g9r1` (a held
  weapon at the ready, by class and stance), and the flinch, fire and swing clips. **There is no draw or holster clip in the game
  files for any class**: checked with `kotor-tool anims S_Male02`.

## Where each weapon sits (the table is `docs/holsters.tsv`)

| Weapon | Hand | Place | Node | Notes |
| --- | --- | --- | --- | --- |
| pistol (4) | right | left hip | `pelvis_g`, left side | butt forward and up, in a holster; a cross draw, as the owner says |
| pistol (4) | left | right hip | `pelvis_g`, right side | the mirror |
| rifle, carbine, bowcaster (5), heavy (6) | either | back, slung | `torsoUpr_g` | across the back, barrel up at the right shoulder |
| blade (2) | right | back, diagonal | `torsoUpr_g` | hilt at the right shoulder, tip at the left hip |
| blade (2) | left | back, diagonal | `torsoUpr_g` | the mirror: hilt at the left shoulder, tip at the right hip |
| two-handed melee (3), double blade | either | back, diagonal | `torsoUpr_g` | **assumed:** as the right-hand blade, centred and a little steeper, since the owner named no place |
| stun baton (1) | right or left | belt | `pelvis_g` | **assumed:** on the belt at the hip of its hand, not the back |

Two blades (right and left) cross on the back as an X. Two pistols sit one on each hip. A pistol in the right hand and a blade in
the left: the pistol on the left hip, the blade on the back, left-handed.

A lightsaber is carried by its hilt: a holstered saber has no blade, only the hilt, at the shoulder end of the diagonal, and a
sheath (a tube) shows the line. **Assumed:** the blade appears only in the hand.

## Drawing and holstering

- **When:** entering combat mode draws, leaving it holsters. Combat mode is the single flag the combat builder is merging
  (attack mode on; off after ten seconds, or by hand). In a conversation or a cutscene the weapon stays holstered unless a script
  asks for it. A creature that fights draws when its fight starts and holsters when it ends, as the player does.
- **Draw clip:** `equip`, played in 0.5 s: the weapon leaves the holster at 40 per cent of the clip and is in the hand from
  there. Holster is the same clip played backwards, the weapon in the hand until 60 per cent and in the holster after. The reach
  wanted is to the hip for a pistol and to the shoulder for a blade or rifle; the stock `equip` has neither. **Assumed:** stage 2
  builds those two reaches as hand-made poses laid over the arm nodes (as `bent_pose` does for the hand), with `equip` playing under
  them for the rest of the body. Their timing is the same.
- **Firing and striking need a drawn weapon.** A creature in combat mode with its weapon still in the holster draws first (about
  0.4 s), and a blow ordered during the draw waits for it. A flinch, a push or a knockdown interrupts a draw: the weapon is in the
  hand at once, as a creature who is hit reaches for it.
- **Creatures without hands** (animals, droids built without arms, the rancor): nothing is drawn, nothing is shown. A creature
  with natural weapons (claws, bite, `baseitems.2da` rows 81 to 84) shows none.
- **Hides:** a creature in `hideequippeditems` of its appearance shows no holster either.

## Stages, one tip each

0. This design, read by Top Dog and the combat builder.
1. A weapon that is not in the hand is drawn at its holster place, following the animation, with the plain holster shapes.
2. The draw and holster motion for each class, and the offhand versions.
3. The combat flag drives it; fire and strike wait for a drawn weapon; conversations and cutscenes holster.
4. Save and load, NPCs and party, Modern and Classic.

Tests for each: the stored weapon is at the place for its class (positions in the node's frame against the table), the held weapon
is at the hand, a clip's draw puts it there by the time the table says, and hidden strips (front, side, back; each class) written
with `FRAME_STRIP`.

## Assumed (say if wrong)

- "Right-handed blaster on the left hip" is a cross draw, as the owner says it, and not a mix-up of sides.
- Rifles are slung across the back in the same place for every body, with no scaling.
- A baton goes on the belt, not the back.
- No new weapon models; the holster shapes are plain.
- Droids with arms (HK-47, the war droids) holster too where their nodes allow (`pelvis_g` and `torso_g`); a droid without those
  nodes keeps the weapon in the hand.
- The offsets in `docs/holsters.tsv` are first guesses, tuned from frames in stage 1.

## 9 Nov: blades along z, more detail, cutscene actors

- **Axis (the cause of the sword bugs):** a sword, saber, double blade and baton model runs along its own **z** (vibroblade z -0.24 to 1.05); a gun runs along **y**. The table had turned every weapon as a gun, so a sword lay across the shoulders, forward and back, as in the owner's screenshots. The blade, twohand and baton rows now turn z: blades hang from the shoulder to the opposite hip (pitch 205 / 155), a double blade lies diagonally across the back at 0.6 of its size (`scale`, a new last column of the table), a baton hangs down from the hip.
- **Detail:** hip holster has a flap, a belt loop, a thigh strap and buckle; the sling has two loops, a buckle and a butt pad; the sheath has two bands and a belt tab; the double blade has a harness. All are boxes in `shape_mesh`. A shape named `_z` is the same turned to run along z. Assumed: the look, from frames.
- **Rifle and heavy:** moved to the middle of the back, leaning left at the top (-22 deg), so the butt does not hang at the left hand. Pistols sit 1 cm further out and 3 cm lower and back, clear of the swinging fist.
- **Cutscenes:** in a conversation or cutscene a weapon is in the hand when the creature plays a weapon's clip (`weapon_clip`: `g5r1`, `b5a1`, `c2a1`) or the scene stages it with a stunt animation and it is not the player, speaker or listener. The talkers keep theirs on the body. Assumed: the stunt animations of the Endar Spire's firing soldiers are not named by their weapon, so any staged actor is armed.
- **Not reproduced:** the first-encounter cutscene itself (needs the route to it). The soldiers are shown standing and running with their gear instead.
