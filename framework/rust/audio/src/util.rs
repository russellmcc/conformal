use super::{Buffer, BufferMut, ChannelLayout};

struct SlicedBuffer<'a, B, I> {
    buffer: &'a B,
    index: I,
}

pub trait BufferIndex {
    fn num_frames(&self, parent: usize) -> usize;
    fn index<'a>(&self, buffer: &'a [f32]) -> &'a [f32];
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

pub fn slice_buffer<'a, B: Buffer, I: BufferIndex + 'a>(
    buffer: &'a B,
    index: I,
) -> impl Buffer + '_ {
    SlicedBuffer { buffer, index }
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

pub fn slice_buffer_mut<'a, B: BufferMut, I: BufferIndex + 'a>(
    buffer: &'a mut B,
    index: I,
) -> impl BufferMut + '_ {
    SlicedMutBuffer { buffer, index }
}

#[cfg(test)]
mod tests;
