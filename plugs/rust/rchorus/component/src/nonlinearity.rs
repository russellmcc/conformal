//! BBD Non-linearity model from [Practical Modeling of Bucket-Brigade Device Circuits](https://www.dafx.de/paper-archive/2010/DAFx10/RaffelSmith_DAFx10_P42.pdf)

// Note the paper has coefficients measured on 1024 stage devices, but
// the roland synth chorus only has 256 stages, so reduce the coefficients
// for less nonlinearity.
const A: f32 = 1. / 8. / 32.;
const B: f32 = 1. / 18. / 32.;

pub fn nonlinearity(x: f32) -> f32 {
    if x > 1. {
        x - A - B
    } else if x < -1. {
        x - A + B
    } else {
        let x2 = x * x;
        // Note that the paper has a spurious "+A" here, but we don't want to add a DC offset
        // and adding this would make the clipping discontinuous, so likely the paper has a typo.
        x - A * x2 - B * x2 * x
    }
}
