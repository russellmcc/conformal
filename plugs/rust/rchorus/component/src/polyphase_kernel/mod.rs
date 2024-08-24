#[derive(Clone)]
pub struct PolyphaseKernel {
    phases: Vec<Vec<f32>>,
}

impl PolyphaseKernel {
    pub fn split(kernel: &[f32], num_phases: u16) -> Self {
        assert!(kernel.len() % num_phases as usize == 0);
        let phase_len = kernel.len() / num_phases as usize;
        let mut phases = vec![vec![0f32; phase_len]; num_phases as usize];
        for (i, tap) in kernel.iter().enumerate() {
            phases[num_phases as usize - 1 - i % num_phases as usize]
                [phase_len - 1 - i / num_phases as usize] = *tap;
        }
        Self { phases }
    }

    pub fn phases(&self) -> u16 {
        u16::try_from(self.phases.len()).unwrap()
    }

    // Note that the phase will be reversed so that applying
    // the kernel is a simple dot product.
    pub fn phase(&self, phase: u16) -> &[f32] {
        &self.phases[phase as usize]
    }
}
