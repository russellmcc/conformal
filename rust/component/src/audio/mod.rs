//! Types and utilities for Audio Buffers.
//!
//! In Conformal, components process audio in buffers. Buffers are groups of samples
//! arranged into channels. In Conformal, each channel is represented by a `&[f32]`.

/// Defines the layout of the channels in a buffer.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ChannelLayout {
    /// A single channel buffer.
    Mono,

    /// A two channel buffer.
    ///
    /// Channel 0 is the left channel, and channel 1 is the right channel.
    Stereo,
}

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

pub mod utils;

impl ChannelLayout {
    /// The number of channels in the layout.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::audio::ChannelLayout;
    /// assert_eq!(ChannelLayout::Mono.num_channels(), 1);
    /// assert_eq!(ChannelLayout::Stereo.num_channels(), 2);
    /// ```
    #[must_use]
    pub fn num_channels(self) -> usize {
        match self {
            ChannelLayout::Mono => 1,
            ChannelLayout::Stereo => 2,
        }
    }
}

/// Represents a (potentially multi-channel) buffer of audio samples
///
/// A [Buffer] doesn't specify the exact storage format of the samples, but
/// each channel must be a contiguous slice of samples. All channels must have
/// the same number of samples, that is, [`Buffer::num_frames`].
pub trait Buffer {
    /// The layout of the channels in the buffer.
    fn channel_layout(&self) -> ChannelLayout;

    /// The number of channels in the buffer.
    fn num_channels(&self) -> usize {
        self.channel_layout().num_channels()
    }

    /// The number of frames in the buffer.
    ///
    /// Each channel will contain this many samples.
    fn num_frames(&self) -> usize;

    /// Get a channel from the buffer.
    ///
    /// This returns a slice that contains all samples of the channel.
    /// The every channel will have [`Self::num_frames()`] elements.
    ///
    /// # Panics
    ///
    /// Panics if `channel` is greater than or equal to [`Self::num_channels()`].
    fn channel(&self, channel: usize) -> &[f32];
}

/// Iterates over the channels of a buffer.
///
/// # Examples
/// ```
/// # use conformal_component::audio::{BufferData, Buffer, channels};
/// let buffer = BufferData::new_stereo([1.0, 2.0], [3.0, 4.0]);
/// assert!(channels(&buffer).eq([[1.0, 2.0], [3.0, 4.0]]));
/// ```
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

    #[must_use]
    pub fn new_mono(data: Vec<f32>) -> BufferData {
        Self {
            channel_layout: ChannelLayout::Mono,
            num_frames: data.len(),
            data,
        }
    }

    #[must_use]
    pub fn new_stereo<L: IntoIterator<Item = f32>, R: IntoIterator<Item = f32>>(
        left: L,
        right: R,
    ) -> BufferData {
        let mut data: Vec<_> = left.into_iter().collect();
        let left_len = data.len();
        data.extend(right);
        assert_eq!(left_len * 2, data.len());
        Self {
            channel_layout: ChannelLayout::Stereo,
            num_frames: left_len,
            data,
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
