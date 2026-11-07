//! When in a ranged attack's clip each shot leaves the gun (the owner's bug of 7 Nov 2026: the animation shows a burst of
//! three and a single bolt came out at the start). The clips carry no events, so the moments are read from the clip's own motion:
//! the kicks of the hands and head, merged when within 0.2 s of each other, each at its peak, over the humanoid supermodel
//! (`fire_frames` in `kotor-model`'s examples prints them).
//!
//! **Measured:** the rifle and heavy clips (`b7a1`, `b9a1`) kick three times, at 0.23, 0.70 and 1.23 s; the pistol (`b5a1`)
//! twice, at 0.53 and 1.03 s; two pistols (`b6a1`) twice, at 0.43 and 1.03 s. **Assumed:** a kick is a shot.

/// The seconds into the clip at which each shot is fired. A clip not listed fires one shot at `DEFAULT_SHOT`.
pub fn fire_times(clip: &str) -> &'static [f32] {
    match clip {
        "b5a1" => &[0.53, 1.03],
        "b6a1" => &[0.43, 1.03],
        "b7a1" | "b9a1" => &[0.23, 0.70, 1.23],
        _ => &[DEFAULT_SHOT],
    }
}

/// Where the one shot of a ranged clip with no table falls.
pub const DEFAULT_SHOT: f32 = 0.3;

/// Whether this is a clip of a ranged attack: `b`, a stance digit, `a` and a number.
pub fn is_ranged_attack(clip: &str) -> bool {
    let b = clip.as_bytes();
    b.len() == 4 && b[0] == b'b' && b[1].is_ascii_digit() && b[2] == b'a'
}
