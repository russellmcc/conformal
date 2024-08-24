use std::ops::Range;

/// `LookBehind` is a low-level building block for delay-based processors.
///
/// Given a buffer, it will return a view into the buffer including some
/// number of previous samples of the stream.
#[derive(Clone)]
pub struct LookBehind {
    // Implementation note: the buffer is stored as a single Vec,
    // and we always keep the audio data contiguous.  In benchmarks
    // this approach tends to be faster than using a `VecDeque`,
    // presumably because having contiguous data when we later
    // look at the buffer makes downstream processing faster, and
    // this makes up for the extra cost of copying the data around
    // when we need to shift it.
    buffer: Vec<f32>,

    look_behind: usize,
}

struct LookBehindView<'a> {
    parent: &'a mut LookBehind,
    input_size: usize,
}

pub trait SliceLike {
    #[allow(dead_code)]
    fn iter(&self) -> impl Iterator<Item = &'_ f32>;
    fn range(&self, range: Range<usize>) -> impl Iterator<Item = &'_ f32>;
}

impl LookBehind {
    pub fn new(look_behind: usize, max_samples_per_process_call: usize) -> Self {
        Self {
            buffer: vec![0.0; look_behind + max_samples_per_process_call],
            look_behind,
        }
    }

    pub fn process<I: std::iter::IntoIterator<Item = f32>>(
        &mut self,
        input: I,
    ) -> impl SliceLike + '_ {
        let input_iter = input.into_iter();
        let mut input_size = 0;
        for (src, dest) in input_iter.zip(self.buffer[self.look_behind..].iter_mut()) {
            *dest = src;
            input_size += 1;
        }

        LookBehindView {
            parent: self,
            input_size,
        }
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
    }
}

impl SliceLike for LookBehindView<'_> {
    fn iter(&self) -> impl Iterator<Item = &'_ f32> {
        self.parent.buffer[..self.input_size + self.parent.look_behind].iter()
    }

    fn range(&self, range: Range<usize>) -> impl Iterator<Item = &'_ f32> {
        self.parent.buffer[range].iter()
    }
}

impl Drop for LookBehindView<'_> {
    fn drop(&mut self) {
        let src = self.input_size..(self.input_size + self.parent.look_behind);
        // consume the input samples
        self.parent.buffer.copy_within(src, 0);
    }
}

#[cfg(test)]
mod tests;
