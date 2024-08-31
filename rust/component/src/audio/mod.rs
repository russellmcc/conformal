#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ChannelLayout {
    Mono,
    Stereo,
}

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

pub mod util;

impl ChannelLayout {
    #[must_use]
    pub fn num_channels(self) -> usize {
        match self {
            ChannelLayout::Mono => 1,
            ChannelLayout::Stereo => 2,
        }
    }
}

pub trait Buffer {
    fn channel_layout(&self) -> ChannelLayout;
    fn num_channels(&self) -> usize {
        self.channel_layout().num_channels()
    }
    fn num_frames(&self) -> usize;

    fn channel(&self, channel: usize) -> &[f32];
}

pub fn channels<B: Buffer>(buffer: &B) -> impl Iterator<Item = &[f32]> {
    (0..buffer.num_channels()).map(move |channel| buffer.channel(channel))
}

pub trait BufferMut: Buffer {
    fn channel_mut(&mut self, channel: usize) -> &mut [f32];
}

pub fn channels_mut<B: BufferMut>(buffer: &mut B) -> impl Iterator<Item = &mut [f32]> {
    (0..buffer.num_channels()).map(move |channel| unsafe {
        std::slice::from_raw_parts_mut(
            buffer.channel_mut(channel).as_mut_ptr(),
            buffer.num_frames(),
        )
    })
}

#[derive(Debug, Clone)]
pub struct BufferData {
    channel_layout: ChannelLayout,
    num_frames: usize,
    data: Vec<f32>,
}

impl BufferData {
    #[must_use]
    pub fn new(channel_layout: ChannelLayout, num_frames: usize) -> Self {
        Self {
            channel_layout,
            num_frames,
            data: vec![0f32; channel_layout.num_channels() * num_frames],
        }
    }
}

impl Buffer for BufferData {
    fn channel_layout(&self) -> ChannelLayout {
        self.channel_layout
    }

    fn num_frames(&self) -> usize {
        self.num_frames
    }

    fn channel(&self, channel: usize) -> &[f32] {
        &self.data[channel * self.num_frames..(channel + 1) * self.num_frames]
    }
}

impl BufferMut for BufferData {
    fn channel_mut(&mut self, channel: usize) -> &mut [f32] {
        &mut self.data[channel * self.num_frames..(channel + 1) * self.num_frames]
    }
}
