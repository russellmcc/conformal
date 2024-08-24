use num_traits::cast;

/// Applies a Hamming window to the data.
///
/// # Panics
/// If the data is longer than 2^23 samples (the most that can be represented
/// in a 32-bit float).
pub fn hamming(data: &mut [f32]) {
    // note that this is totally unoptimized and rounding errors
    // may cause non-symmetry :'(.

    let increment = std::f32::consts::TAU / cast::<usize, f32>(data.len()).unwrap();
    for (index, sample) in data.iter_mut().enumerate() {
        *sample *= 0.54 - 0.46 * (cast::<usize, f32>(index).unwrap() * increment).cos();
    }
}

/// Applies a Blackman-Harris window to the data.
///
/// # Panics
/// If the data is longer than 2^23 samples (the most that can be represented
/// in a 32-bit float).
pub fn blackman_harris(data: &mut [f32]) {
    let increment = std::f32::consts::TAU / cast::<usize, f32>(data.len()).unwrap();
    for (index, sample) in data.iter_mut().enumerate() {
        let x = cast::<usize, f32>(index).unwrap() * increment;
        *sample *=
            0.35875 - 0.48829 * x.cos() + 0.14128 * (2.0 * x).cos() - 0.01168 * (3.0 * x).cos();
    }
}

/// Applies an exponential approximation to the DPSS window to the data.
///
/// This is similar to a Kaiser window, but with a different alpha parameter.
///
/// Higher alpha values will create more side-band attenuation, but will have
/// a wider main lobe.
///
/// # Panics
/// If the data is longer than 2^23 samples (the most that can be represented
/// in a 32-bit float).
pub fn dpss_approx_exp(buffer: &mut [f32], alpha: f32) {
    let n = buffer.len();
    let scale = 1f32 / alpha.exp();
    let i_scale = 1f32 / (cast::<usize, f32>(n).unwrap() - 1f32);
    for (i, x) in buffer.iter_mut().enumerate() {
        // shift i to be centered around 0, ranging -0.5 to 0.5
        let i = cast::<usize, f32>(i).unwrap() * i_scale - 0.5;
        *x *= scale * ((alpha) * (1f32 - i * i).sqrt()).exp();
    }
}
