//! Utilities for slicing audio buffers.

// this is a private module
#![allow(clippy::module_name_repetitions)]

use super::{Buffer, BufferMut, ChannelLayout};

struct SlicedBuffer<'a, B, I> {
    buffer: &'a B,
    index: I,
}

/// A trait for index types that can be used in `slice_buffer` and `slice_buffer_mut`.
pub trait BufferIndex {
    #[doc(hidden)]
    fn num_frames(&self, parent: usize) -> usize;
    #[doc(hidden)]
    fn index<'a>(&self, buffer: &'a [f32]) -> &'a [f32];
    #[doc(hidden)]
    fn index_mut<'a>(&self, buffer: &'a mut [f32]) -> &'a mut [f32];
}

impl BufferIndex for std::ops::Range<usize> {
    fn num_frames(&self, _parent: usize) -> usize {
        self.len()
    }

    fn index<'a>(&self, buffer: &'a [f32]) -> &'a [f32] {
        &buffer[self.clone()]
    }

    fn index_mut<'a>(&self, buffer: &'a mut [f32]) -> &'a mut [f32] {
        &mut buffer[self.clone()]
    }
}

impl BufferIndex for std::ops::RangeInclusive<usize> {
    fn num_frames(&self, _parent: usize) -> usize {
        self.end() - self.start() + 1
    }

    fn index<'a>(&self, buffer: &'a [f32]) -> &'a [f32] {
        &buffer[self.clone()]
    }

    fn index_mut<'a>(&self, buffer: &'a mut [f32]) -> &'a mut [f32] {
        &mut buffer[self.clone()]
    }
}

impl BufferIndex for std::ops::RangeFrom<usize> {
    fn num_frames(&self, parent: usize) -> usize {
        parent - self.start
    }

    fn index<'a>(&self, buffer: &'a [f32]) -> &'a [f32] {
        &buffer[self.clone()]
    }

    fn index_mut<'a>(&self, buffer: &'a mut [f32]) -> &'a mut [f32] {
        &mut buffer[self.clone()]
    }
}

impl BufferIndex for std::ops::RangeTo<usize> {
    fn num_frames(&self, _parent: usize) -> usize {
        self.end
    }

    fn index<'a>(&self, buffer: &'a [f32]) -> &'a [f32] {
        &buffer[*self]
    }

    fn index_mut<'a>(&self, buffer: &'a mut [f32]) -> &'a mut [f32] {
        &mut buffer[*self]
    }
}

impl BufferIndex for std::ops::RangeFull {
    fn num_frames(&self, parent: usize) -> usize {
        parent
    }

    fn index<'a>(&self, buffer: &'a [f32]) -> &'a [f32] {
        buffer
    }

    fn index_mut<'a>(&self, buffer: &'a mut [f32]) -> &'a mut [f32] {
        buffer
    }
}

impl BufferIndex for std::ops::RangeToInclusive<usize> {
    fn num_frames(&self, _parent: usize) -> usize {
        self.end + 1
    }

    fn index<'a>(&self, buffer: &'a [f32]) -> &'a [f32] {
        &buffer[*self]
    }

    fn index_mut<'a>(&self, buffer: &'a mut [f32]) -> &'a mut [f32] {
        &mut buffer[*self]
    }
}

impl<B: Buffer, I: BufferIndex> Buffer for SlicedBuffer<'_, B, I> {
    fn channel_layout(&self) -> ChannelLayout {
        self.buffer.channel_layout()
    }

    fn num_frames(&self) -> usize {
        self.index.num_frames(self.buffer.num_frames())
    }

    fn channel(&self, channel: usize) -> &[f32] {
        self.index.index(self.buffer.channel(channel))
    }
}

/// Create a sub-buffer from a buffer using an index range.
///
/// # Examples
///
/// ```
/// # use conformal_component::audio::{Buffer, BufferData};
/// # use conformal_component::audio::slice_buffer;
/// let buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0]);
/// assert_eq!(slice_buffer(&buffer, 1..).channel(0), [2.0, 3.0]);
/// assert_eq!(slice_buffer(&buffer, 1..2).channel(0), [2.0]);
/// assert_eq!(slice_buffer(&buffer, ..=1).channel(0), [1.0, 2.0]);
/// ```
///
/// # Panics
///
/// Will panic if the index range isn't within the bounds of the buffer:
///
/// ```should_panic
/// # use conformal_component::audio::{BufferData};
/// # use conformal_component::audio::slice_buffer;;
/// let buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0]);
/// slice_buffer(&buffer, 4..);
/// ```
pub fn slice_buffer<'a, B: Buffer, I: BufferIndex + 'a>(
    buffer: &'a B,
    index: I,
) -> impl Buffer + '_ {
    let ret = SlicedBuffer { buffer, index };
    // Grab the first channel and throw it away - this will
    // cause us to panic early in the case of an invalid range
    ret.channel(0);
    ret
}

struct SlicedMutBuffer<'a, B, I> {
    buffer: &'a mut B,
    index: I,
}

impl<B: Buffer, I: BufferIndex> Buffer for SlicedMutBuffer<'_, B, I> {
    fn channel_layout(&self) -> ChannelLayout {
        self.buffer.channel_layout()
    }

    fn num_frames(&self) -> usize {
        self.index.num_frames(self.buffer.num_frames())
    }

    fn channel(&self, channel: usize) -> &[f32] {
        self.index.index(self.buffer.channel(channel))
    }
}

impl<B: BufferMut, I: BufferIndex> BufferMut for SlicedMutBuffer<'_, B, I> {
    fn channel_mut(&mut self, channel: usize) -> &mut [f32] {
        self.index.index_mut(self.buffer.channel_mut(channel))
    }
}

/// Create a sub-buffer of a mutable buffer using an index range.
///
/// # Examples
///
/// ```
/// # use conformal_component::audio::{Buffer, BufferMut, BufferData};
/// # use conformal_component::audio::slice_buffer_mut;
/// let mut buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0]);
/// slice_buffer_mut(&mut buffer, 1..).channel_mut(0).copy_from_slice(&[4.0, 5.0]);
/// assert_eq!(buffer.channel(0), [1.0, 4.0, 5.0]);
/// ```
///
/// # Panics
///
/// Will panic if the index range isn't within the bounds of the buffer:
///
/// ```should_panic
/// # use conformal_component::audio::{BufferData};
/// # use conformal_component::audio::slice_buffer_mut;
/// let mut buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0]);
/// slice_buffer_mut(&mut buffer, 4..);
/// ```
pub fn slice_buffer_mut<'a, B: BufferMut, I: BufferIndex + 'a>(
    buffer: &'a mut B,
    index: I,
) -> impl BufferMut + '_ {
    let ret = SlicedMutBuffer { buffer, index };
    // Grab the first channel and throw it away - this will
    // cause us to panic early in the case of an invalid range
    ret.channel(0);
    ret
}

#[cfg(test)]
mod tests;
