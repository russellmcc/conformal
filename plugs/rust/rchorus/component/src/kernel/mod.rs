use num_traits::cast;
use util::window;

/// Create a sinc function with given length and increment
fn sinc(length: usize, increment: f32) -> Vec<f32> {
    let mut sinc = vec![0f32; length];
    let half = if length % 2 == 1 {
        cast::<usize, f32>(length / 2).unwrap()
    } else {
        cast::<usize, f32>(length / 2).unwrap() - 0.5
    };
    let increment = std::f32::consts::TAU * increment;
    for (i, x) in sinc.iter_mut().enumerate() {
        let i = (cast::<usize, f32>(i).unwrap() - half) * increment;
        if i.abs() < 1e-6 {
            *x = 1f32;
        } else {
            *x = i.sin() / i;
        }
    }
    sinc
}

pub struct LpfOptions {
    pub length: u16,
    pub increment: f32,
}
pub fn lpf(LpfOptions { length, increment }: LpfOptions) -> Vec<f32> {
    let mut kernel = sinc(length as usize, increment);
    window::blackman_harris(&mut kernel);
    kernel
}
