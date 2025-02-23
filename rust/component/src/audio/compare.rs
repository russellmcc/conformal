//! Utilities for comparing audio samples, slices, and buffers

use super::{Buffer, channels};
use itertools::{EitherOrBoth, Itertools};

/// Checks if two `f32` values `a` and `b` are within `e` of each other.
///
/// This is useful for comparing floating point values, while allowing for
/// some rounding errors.
///
/// # Examples
///
/// ```
/// # use conformal_component::audio::approx_eq;
/// assert_eq!(approx_eq(1.0, 1.01, 0.1), true);
/// assert_eq!(approx_eq(1.0, 1.3, 0.1), false);
/// ```
#[must_use]
pub fn approx_eq(a: f32, b: f32, e: f32) -> bool {
    (a - b).abs() < e
}

/// Checks if all the values from two iterators of f32 values are within `e` of each other.
///
/// This is useful for comparing ranges of floating point values, while allowing for
/// some rounding errors.
///
/// # Examples
///
/// ```
/// # use conformal_component::audio::all_approx_eq;
/// assert!(all_approx_eq([1.0, 2.0, 3.0], [1.01, 2.01, 3.01], 0.1));
/// assert!(!all_approx_eq([1.0, 2.0, 3.0], [1.01, 2.2, 3.01], 0.1));
/// assert!(!all_approx_eq([1.0, 2.0, 3.0], [1.0, 2.0], 0.1));
/// ```
#[must_use]
pub fn all_approx_eq<L: IntoIterator<Item = f32>, R: IntoIterator<Item = f32>>(
    lhs: L,
    rhs: R,
    e: f32,
) -> bool {
    lhs.into_iter().zip_longest(rhs).all(|x| match x {
        EitherOrBoth::Both(l, r) => approx_eq(l, r, e),
        _ => false,
    })
}

/// Checks two buffers are equal to within a tolerance `e`.
///
/// Note that buffers will only count as equal if they have
/// the same channel layout and length, and if all samples
/// are within `e` of each other.
///
/// # Examples
///
/// ```
/// # use conformal_component::audio::{BufferData, Buffer, buffer_approx_eq};
/// assert!(buffer_approx_eq(
///   &BufferData::new_mono(vec![1.0, 2.0, 3.0]),
///   &BufferData::new_mono(vec![1.01, 2.01, 3.01]),
///   0.1));
/// assert!(!buffer_approx_eq(
///   &BufferData::new_mono(vec![1.0, 2.0, 3.0]),
///   &BufferData::new_mono(vec![1.01, 2.2, 3.01]),
///   0.1));
/// assert!(!buffer_approx_eq(
///   &BufferData::new_mono(vec![1.0, 2.0, 3.0]),
///   &BufferData::new_mono(vec![1.0, 2.0]),
///   0.1));
/// assert!(!buffer_approx_eq(
///   &BufferData::new_stereo(vec![1.0, 2.0], vec![3.0, 4.0]),
///   &BufferData::new_mono(vec![1.0, 2.0, 3.0, 4.0]),
///   0.1));
/// ```
#[must_use]
pub fn buffer_approx_eq<A: Buffer, B: Buffer>(a: &A, b: &B, e: f32) -> bool {
    a.channel_layout() == b.channel_layout()
        && all_approx_eq(
            channels(a).flatten().copied(),
            channels(b).flatten().copied(),
            e,
        )
}
