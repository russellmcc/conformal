//! Utilities only useful for tests

/// Checks if two f32 values `a` and `b` are within `e` of each other.
///
/// This is useful for comparing floating point values, while allowing for
/// some rounding errors.
///
/// # Examples
///
/// ```
/// # use conformal_component::audio::test_utils::samplewise_diff;
/// assert_eq!(samplewise_diff(1.0, 1.01, 0.1), true);
/// assert_eq!(samplewise_diff(1.0, 1.3, 0.1), false);
/// ```
pub fn samplewise_diff(a: f32, b: f32, e: f32) -> bool {
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
/// # use conformal_component::audio::test_utils::samplewise_diff_iters;
/// assert_eq!(samplewise_diff_iters([1.0, 2.0, 3.0], [1.01, 2.01, 3.01], 0.1), true);
/// assert_eq!(samplewise_diff_iters([1.0, 2.0, 3.0], [1.01, 2.2, 3.01], 0.1), false);
/// ```
pub fn samplewise_diff_iters<I: IntoIterator<Item = f32>, J: IntoIterator<Item = f32>>(
    i: I,
    j: J,
    e: f32,
) -> bool {
    i.into_iter().zip(j).all(|(a, b)| samplewise_diff(a, b, e))
}
