//! We emulate the SKF used as anti-aliasing/reconstruction filters
//! with SVFs with the same frequency response. It would be fun to write
//! up an application note on this.

use iir::svf::{calc_g, Input, RawParams, Svf};

#[derive(Debug, Clone)]
pub struct AntiAliasingFilter {
    g: Option<f64>,

    a: Svf,
    a_damping: f64,

    b: Svf,
    b_damping: f64,
}

const CUTOFF: f32 = 10_000.;

impl AntiAliasingFilter {
    pub fn new(sampling_rate: f32) -> Self {
        // We use a simple butterworth filter
        Self {
            g: if sampling_rate > CUTOFF * 2.2 {
                let incr = CUTOFF / sampling_rate;
                Some(calc_g(f64::from(incr)))
            } else {
                None
            },
            a: Default::default(),
            a_damping: 2. * (std::f64::consts::TAU / 4. * (3. / 4.)).cos(),
            b: Default::default(),
            b_damping: 2. * (std::f64::consts::TAU / 4. / 4.).cos(),
        }
    }

    pub fn process<'a, I: IntoIterator<Item = f32> + 'a>(
        &'a mut self,
        input: I,
    ) -> impl Iterator<Item = f32> + 'a {
        let g = self.g.unwrap_or(0.);
        let active = self.g.is_some();
        let a_params = RawParams {
            g,
            two_r: self.a_damping,
        };
        let b_params = RawParams {
            g,
            two_r: self.b_damping,
        };
        let a = &mut self.a;
        let b = &mut self.b;
        #[allow(clippy::cast_possible_truncation)]
        b.process_low(
            a.process_low(input.into_iter().map(move |x| Input {
                x: if active { f64::from(x) } else { 0. },
                params: a_params,
            }))
            .map(move |x| Input {
                x,
                params: b_params,
            }),
        )
        .map(|x| x as f32)
    }

    pub fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
    }
}
