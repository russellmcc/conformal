use std::ops::RangeInclusive;

#[must_use]
pub fn rescale_points(input: f32, from_low: f32, from_high: f32, to_low: f32, to_high: f32) -> f32 {
    let input = (input - from_low) / (from_high - from_low);
    input * (to_high - to_low) + to_low
}

#[must_use]
pub fn rescale(input: f32, from: RangeInclusive<f32>, to: RangeInclusive<f32>) -> f32 {
    rescale_points(input, *from.start(), *from.end(), *to.start(), *to.end())
}

#[must_use]
pub fn rescale_inverted(input: f32, from: RangeInclusive<f32>, to: RangeInclusive<f32>) -> f32 {
    rescale_points(input, *from.start(), *from.end(), *to.end(), *to.start())
}

#[must_use]
pub fn rescale_clamped(input: f32, from: RangeInclusive<f32>, to: RangeInclusive<f32>) -> f32 {
    rescale(input, from.clone(), to.clone()).clamp(*to.start(), *to.end())
}

#[must_use]
pub fn rescale_inverted_clamped(
    input: f32,
    from: RangeInclusive<f32>,
    to: RangeInclusive<f32>,
) -> f32 {
    rescale_inverted(input, from.clone(), to.clone()).clamp(*to.start(), *to.end())
}

#[must_use]
pub fn lerp(value_at_zero: f32, value_at_one: f32, t: f32) -> f32 {
    value_at_zero * (1.0 - t) + value_at_one * t
}

#[must_use]
pub fn lerp_clamped(value_at_zero: f32, value_at_one: f32, t: f32) -> f32 {
    lerp(value_at_zero, value_at_one, t.clamp(0.0, 1.0))
}
