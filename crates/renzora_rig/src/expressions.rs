//! Facial expressions as bone offsets: the face rig's built-in sliders.
//!
//! In the editor these are the face preset of the Controls tab
//! ([`crate::controls::face_preset`] writes them out as ordinary, editable
//! controls); generated blinks and lip sync still key from this table
//! directly, since they need to know which slider is the jaw and which the
//! lids whatever the user has renamed.
//!
//! Each expression moves some face bones away from their rest pose, by an
//! amount proportional to the slider: a rotation (degrees about the bone's own
//! X, Y, Z) and a translation in units of the face's scale (the distance
//! between the eyes), so the same table fits a child's head and a giant's.
//! The axes are the face rig's: X toward the character's left, Y up, Z
//! forward (see [`crate::face`]).
//!
//! Several expressions add together, so Smile and Jaw Open at once is a smile
//! with the mouth open. Rotations compose, translations sum.

use bevy::math::{EulerRot, Quat, Vec3};

use crate::face::names;

/// One bone's part in an expression, at full strength.
#[derive(Clone, Copy, Debug)]
pub struct Offset {
    pub bone: &'static str,
    /// Degrees about X, Y, Z.
    pub rotate: [f32; 3],
    /// In face-scale units.
    pub translate: [f32; 3],
}

const fn rot(bone: &'static str, x: f32, y: f32, z: f32) -> Offset {
    Offset { bone, rotate: [x, y, z], translate: [0.0; 3] }
}

const fn mv(bone: &'static str, x: f32, y: f32, z: f32) -> Offset {
    Offset { bone, rotate: [0.0; 3], translate: [x, y, z] }
}

/// A slider.
#[derive(Clone, Copy, Debug)]
pub struct Expression {
    pub id: &'static str,
    pub label: &'static str,
    /// `-1..1` for a slider with two directions (look left/right), else `0..1`.
    pub signed: bool,
    pub offsets: &'static [Offset],
}

/// Every expression, in the Face panel's order.
pub const EXPRESSIONS: &[Expression] = &[
    Expression { id: "jaw_open", label: "Jaw Open", signed: false, offsets: &[rot(names::JAW, 24.0, 0.0, 0.0)] },
    Expression {
        id: "blink_l",
        label: "Blink Left",
        signed: false,
        offsets: &[rot(names::LID_UPPER_L, 36.0, 0.0, 0.0), rot(names::LID_LOWER_L, -8.0, 0.0, 0.0)],
    },
    Expression {
        id: "blink_r",
        label: "Blink Right",
        signed: false,
        offsets: &[rot(names::LID_UPPER_R, 36.0, 0.0, 0.0), rot(names::LID_LOWER_R, -8.0, 0.0, 0.0)],
    },
    Expression {
        id: "smile",
        label: "Smile",
        signed: false,
        offsets: &[
            mv(names::CORNER_L, 0.07, 0.12, -0.04),
            mv(names::CORNER_R, -0.07, 0.12, -0.04),
            mv(names::LIP_UPPER, 0.0, 0.025, 0.0),
            rot(names::LID_LOWER_L, -6.0, 0.0, 0.0),
            rot(names::LID_LOWER_R, -6.0, 0.0, 0.0),
        ],
    },
    Expression {
        id: "frown",
        label: "Frown",
        signed: false,
        offsets: &[mv(names::CORNER_L, 0.02, -0.1, 0.0), mv(names::CORNER_R, -0.02, -0.1, 0.0)],
    },
    Expression {
        id: "pucker",
        label: "Pucker",
        signed: false,
        offsets: &[
            mv(names::CORNER_L, -0.09, 0.0, 0.05),
            mv(names::CORNER_R, 0.09, 0.0, 0.05),
            mv(names::LIP_UPPER, 0.0, 0.0, 0.06),
            mv(names::LIP_LOWER, 0.0, 0.0, 0.06),
        ],
    },
    Expression {
        id: "mouth_wide",
        label: "Mouth Wide",
        signed: false,
        offsets: &[mv(names::CORNER_L, 0.1, 0.0, -0.02), mv(names::CORNER_R, -0.1, 0.0, -0.02)],
    },
    Expression {
        id: "lips_part",
        label: "Lips Part",
        signed: false,
        offsets: &[mv(names::LIP_UPPER, 0.0, 0.05, 0.01), mv(names::LIP_LOWER, 0.0, -0.05, 0.01)],
    },
    Expression {
        id: "brows_up",
        label: "Brows Up",
        signed: false,
        offsets: &[mv(names::BROW_L, 0.0, 0.12, 0.0), mv(names::BROW_R, 0.0, 0.12, 0.0)],
    },
    Expression {
        id: "brows_down",
        label: "Brows Down",
        signed: false,
        offsets: &[mv(names::BROW_L, -0.04, -0.07, 0.01), mv(names::BROW_R, 0.04, -0.07, 0.01)],
    },
    Expression {
        id: "look_side",
        label: "Look Left / Right",
        signed: true,
        offsets: &[rot(names::EYE_L, 0.0, 22.0, 0.0), rot(names::EYE_R, 0.0, 22.0, 0.0)],
    },
    Expression {
        id: "look_up",
        label: "Look Up / Down",
        signed: true,
        offsets: &[
            rot(names::EYE_L, -16.0, 0.0, 0.0),
            rot(names::EYE_R, -16.0, 0.0, 0.0),
            rot(names::LID_UPPER_L, -8.0, 0.0, 0.0),
            rot(names::LID_UPPER_R, -8.0, 0.0, 0.0),
        ],
    },
];

pub fn find(id: &str) -> Option<&'static Expression> {
    EXPRESSIONS.iter().find(|e| e.id == id)
}

/// The combined offset each bone gets from a set of slider values: a local
/// rotation to apply after the rest rotation, and a translation (in model
/// units, already scaled by `face_scale`) to add to the rest translation.
pub fn mix(values: &[(&str, f32)], face_scale: f32) -> Vec<(&'static str, Quat, Vec3)> {
    mix_tuned(values, face_scale, &|_| None)
}

/// How one head's lid closes, measured when it was rigged
/// (`face::EyeOpening`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LidTuning {
    /// Degrees the lid turns to close.
    pub degrees: f32,
    /// How far forward it moves as it closes, in face-scale units, to clear
    /// an eyeball that bulges past the lids.
    pub push: f32,
}

/// [`mix`] for a particular head: `blink` gives a lid bone's [`LidTuning`].
/// A blink then closes exactly that far; a lid with no measurement keeps the
/// stock angle.
pub fn mix_tuned(values: &[(&str, f32)], face_scale: f32, blink: &dyn Fn(&str) -> Option<LidTuning>) -> Vec<(&'static str, Quat, Vec3)> {
    let mut out: Vec<(&'static str, Quat, Vec3)> = Vec::new();
    for (id, value) in values {
        let Some(e) = find(id) else { continue };
        let v = if e.signed { value.clamp(-1.0, 1.0) } else { value.clamp(0.0, 1.0) };
        if v.abs() < 1e-5 {
            continue;
        }
        let blinking = matches!(e.id, "blink_l" | "blink_r");
        for o in e.offsets {
            // The stock angle's sign says which way the lid goes (down for
            // an upper lid, up for a lower one); the head says how far.
            let tuning = blink(o.bone).filter(|_| blinking);
            let x = match tuning {
                Some(t) => t.degrees.abs() * o.rotate[0].signum(),
                None => o.rotate[0],
            };
            // Forward is +Z in the face rig's frame.
            let push = Vec3::Z * tuning.map_or(0.0, |t| t.push);
            let r = Quat::from_euler(
                EulerRot::XYZ,
                (x * v).to_radians(),
                (o.rotate[1] * v).to_radians(),
                (o.rotate[2] * v).to_radians(),
            );
            let t = (Vec3::from_array(o.translate) + push) * v * face_scale;
            match out.iter_mut().find(|(b, ..)| *b == o.bone) {
                Some((_, q, tr)) => {
                    *q = (*q * r).normalize();
                    *tr += t;
                }
                None => out.push((o.bone, r, t)),
            }
        }
    }
    out
}

/// A natural blink's closure (0 open, 1 shut) `t` seconds into it: a quick
/// close, a short hold, a slower open. 0.25 s long.
pub fn blink_curve(t: f32) -> f32 {
    const CLOSE: f32 = 0.07;
    const HOLD: f32 = 0.03;
    const OPEN: f32 = 0.15;
    if t <= 0.0 || t >= CLOSE + HOLD + OPEN {
        0.0
    } else if t < CLOSE {
        crate::skin::smoothstep(0.0, CLOSE, t)
    } else if t < CLOSE + HOLD {
        1.0
    } else {
        1.0 - crate::skin::smoothstep(0.0, OPEN, t - CLOSE - HOLD)
    }
}

/// Length of [`blink_curve`].
pub const BLINK_SECS: f32 = 0.25;

/// Blink start times over `duration` seconds: every 2.5 to 6 seconds, from a
/// seeded generator so the same seed gives the same blinks.
pub fn blink_times(duration: f32, seed: u64) -> Vec<f32> {
    blink_times_between(duration, seed, 2.5, 6.0)
}

/// [`blink_times`] with the gap between blinks drawn from `min..max`
/// seconds instead of the default 2.5 to 6: a nervous character blinks more,
/// one concentrating less.
pub fn blink_times_between(duration: f32, seed: u64, min: f32, max: f32) -> Vec<f32> {
    let min = min.max(BLINK_SECS + 0.05);
    let span = (max - min).max(0.0);
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f32 / (1u64 << 53) as f32
    };
    let mut out = Vec::new();
    let mut t = 0.4 + next() * 1.5;
    while t + BLINK_SECS < duration {
        out.push(t);
        t += min + next() * span;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_expression_names_real_face_bones() {
        let all = [
            names::HEAD, names::JAW, names::LIP_UPPER, names::LIP_LOWER, names::CORNER_L, names::CORNER_R, names::EYE_L,
            names::EYE_R, names::LID_UPPER_L, names::LID_UPPER_R, names::LID_LOWER_L, names::LID_LOWER_R, names::BROW_L,
            names::BROW_R,
        ];
        for e in EXPRESSIONS {
            for o in e.offsets {
                assert!(all.contains(&o.bone), "{} names {}", e.id, o.bone);
            }
        }
    }

    #[test]
    fn expressions_add_up() {
        let both = mix(&[("smile", 1.0), ("pucker", 1.0)], 1.0);
        let corner = both.iter().find(|(b, ..)| *b == names::CORNER_L).expect("corner moves");
        assert!((corner.2.x - (0.07 - 0.09)).abs() < 1e-5);
    }

    /// A head's measured lid travel replaces the stock angle, keeping its
    /// direction: down for the upper lid, up for the lower one.
    #[test]
    fn a_measured_blink_turns_the_lids_that_far() {
        let tuned = |b: &str| match b {
            names::LID_UPPER_L => Some(LidTuning { degrees: 50.0, push: 0.1 }),
            names::LID_LOWER_L => Some(LidTuning { degrees: 12.0, push: 0.0 }),
            _ => None,
        };
        let pose = mix_tuned(&[("blink_l", 1.0)], 1.0, &tuned);
        let angle = |bone: &str| {
            let q = pose.iter().find(|(b, ..)| *b == bone).expect("lid moves").1;
            q.to_euler(EulerRot::XYZ).0.to_degrees()
        };
        assert!((angle(names::LID_UPPER_L) - 50.0).abs() < 1e-3);
        assert!((angle(names::LID_LOWER_L) + 12.0).abs() < 1e-3);
        // The pushed lid comes forward as it closes, scaled by the face.
        let half = mix_tuned(&[("blink_l", 0.5)], 2.0, &tuned);
        let upper = half.iter().find(|(b, ..)| *b == names::LID_UPPER_L).expect("lid moves").2;
        assert!((upper.z - 0.1).abs() < 1e-5, "{upper:?}");
        // Without a measurement, the stock angle.
        let stock = mix(&[("blink_l", 1.0)], 1.0);
        let upper = stock.iter().find(|(b, ..)| *b == names::LID_UPPER_L).expect("lid moves").1;
        assert!((upper.to_euler(EulerRot::XYZ).0.to_degrees() - 36.0).abs() < 1e-3);
    }

    #[test]
    fn a_blink_closes_and_opens() {
        assert_eq!(blink_curve(0.0), 0.0);
        assert!((blink_curve(0.08) - 1.0).abs() < 1e-5);
        assert_eq!(blink_curve(BLINK_SECS), 0.0);
    }

    #[test]
    fn blinks_are_spaced_and_repeatable() {
        let a = blink_times(20.0, 7);
        assert_eq!(a, blink_times(20.0, 7));
        assert!(a.len() >= 3);
        assert!(a.windows(2).all(|w| w[1] - w[0] >= 2.5));
    }
}
