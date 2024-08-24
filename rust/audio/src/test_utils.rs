pub fn samplewise_diff(a: f32, b: f32, e: f32) -> bool {
    (a - b).abs() < e
}

pub fn samplewise_diff_iters<I: Iterator<Item = f32>, J: Iterator<Item = f32>>(
    i: I,
    j: J,
    e: f32,
) -> bool {
    i.zip(j).all(|(a, b)| samplewise_diff(a, b, e))
}
