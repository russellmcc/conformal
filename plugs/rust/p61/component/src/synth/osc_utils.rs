/// This is a second-order lagrange step function residual.
/// It would be nice to have a blog-style derivation of this,
/// but for now the basic idea is that this is a magic signal
/// that we can add to any -1 -> 1 step function to minimally anti-alias
/// it.
pub fn polyblep2_residual(phase: f32, increment: f32) -> f32 {
    if phase < increment {
        // Generate the beginning residual.
        let t = phase / increment;
        -((t - 1.0) * (t - 1.0))
    } else if phase > 1.0 - increment {
        // Generate the ending residual.
        let t = (phase - 1.0) / increment;
        (t + 1.0) * (t + 1.0)
    } else {
        0.0
    }
}

fn rotate(phase: f32, x: f32) -> f32 {
    let phase = phase + (1.0 - x);
    if phase > 1.0 {
        phase - 1.0
    } else {
        phase
    }
}

pub fn pulse(phase: f32, increment: f32, width: f32) -> f32 {
    if width < increment {
        -1.0
    } else if width > 1.0 - increment {
        1.0
    } else {
        (if phase < width { -1.0 } else { 1.0 }) - polyblep2_residual(phase, increment)
            + polyblep2_residual(rotate(phase, width), increment)
    }
}

// Optimization opportunity - this could probably be well approximated
pub fn increment(midi_pitch: f32, sampling_rate: f32) -> f32 {
    440f32 * 2.0f32.powf((midi_pitch - 69f32) / 12f32) / sampling_rate
}
