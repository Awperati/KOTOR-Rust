use kotor_render::{mouth_at, MOUTH_RATE, VOICE_STEPS};

/// Owner, 8 Nov: "facial animations dont lip sync to what they are saying properly, the mouth still moves too fast". A voice of
/// four syllables a second with a pause in the middle: the mouth opens as often as the voice has syllables, is shut in the pause,
/// and the loop of mouth shapes does not run in the pause or at the clip's own speed.
#[test]
fn the_mouth_follows_the_voices_syllables_and_stops_in_a_pause() {
    let steps = VOICE_STEPS as usize;
    // Four syllables a second: 120 ms of voice, 130 ms of quiet. Seconds 0 to 1.5 and 2.5 to 4; none from 1.5 to 2.5.
    let energy: Vec<f32> = (0..4 * steps)
        .map(|i| {
            let t = i as f32 / VOICE_STEPS;
            let voiced = !(1.5..2.5).contains(&t) && (t * 4.0).fract() < 0.48;
            if voiced {
                1.0
            } else {
                0.0
            }
        })
        .collect();
    let said = 4.0;
    let open: Vec<f32> = (0..400)
        .map(|i| mouth_at(&energy, i as f32 / 100.0, said).0)
        .collect();
    // Peaks: where the mouth is open past a half, counted once for each time it goes up through it.
    let peaks = |from: usize, to: usize| {
        open[from..to]
            .windows(2)
            .filter(|w| w[0] < 0.5 && w[1] >= 0.5)
            .count()
    };
    let (a, b) = (peaks(20, 140), peaks(260, 380));
    // 120 steps of voice at 4 a second is 4.8 to 5 syllables.
    assert!((4..=5).contains(&a), "{a} openings in 1.2 s of voice");
    assert!((4..=5).contains(&b), "{b} openings in 1.2 s of voice");
    // The pause: shut, and the loop's clock does not move.
    assert!(
        open[170..230].iter().all(|o| *o < 0.1),
        "{:?}",
        &open[170..180]
    );
    let clock = |t: f32| mouth_at(&energy, t, said).1;
    assert!((clock(2.4) - clock(1.7)).abs() < 0.02);
    // The loop advances at MOUTH_RATE of the clip's speed over the voiced part: a clip second per second of voice, not more.
    let voiced_seconds = energy[..75].iter().filter(|e| **e > 0.0).count() as f32 / VOICE_STEPS;
    // This voice has 8 syllables for each second it is on, a quicker pace than the usual 6: the loop turns a third faster.
    let expected = voiced_seconds * MOUTH_RATE * 8.3 / 6.0;
    assert!(
        (clock(1.5) - expected).abs() < 0.06,
        "{} {expected}",
        clock(1.5)
    );
}

/// The loop of mouth shapes is scaled to the line's own pace: over the same voiced time, a line of 4 syllables a second turns
/// it faster than one of 2, and in step with its syllables.
#[test]
fn a_fast_speaker_turns_the_mouth_faster_than_a_slow_one() {
    let line = |per_second: f32| -> Vec<f32> {
        (0..3 * VOICE_STEPS as usize)
            .map(|i| {
                let t = i as f32 / VOICE_STEPS;
                if (t * per_second).fract() < 0.5 {
                    1.0
                } else {
                    0.0
                }
            })
            .collect()
    };
    let (slow, fast) = (line(2.0), line(4.0));
    let at_end = |e: &[f32]| mouth_at(e, 2.9, 3.0).1;
    let (a, b) = (at_end(&slow), at_end(&fast));
    // Both are voiced for half the time; the fast one is twice as quick.
    assert!(b > a * 1.7 && b < a * 2.3, "slow {a}, fast {b}");
    assert!(a > 0.0);
}
