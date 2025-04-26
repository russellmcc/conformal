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

mod compare;
pub use compare::*;

mod slice;
pub use slice::*;

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
    /// The every channel will have [`Self::num_frames`] elements.
    ///
    /// # Panics
    ///
    /// Panics if `channel` is greater than or equal to [`Self::num_channels`].
    fn channel(&self, channel: usize) -> &[f32];
}

/// Returns an iterator for the channels of a buffer.
///
/// The items of this iterator will be slices of the samples of each channel.
/// Each slice will be exactly [`Buffer::num_frames`] elements long.
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

/// A mutable (potentially multi-channel) buffer of audio samples.
///
/// This is a mutable version of [`Buffer`].
pub trait BufferMut: Buffer {
    /// Get a channel from the buffer as a mutable slice
    fn channel_mut(&mut self, channel: usize) -> &mut [f32];
}

/// Returns an iterator for the channels of a mutable buffer.
///
/// The items of this iterator will be mutable slices of the samples of each channel.
///
/// # Examples
///
/// ```
/// # use conformal_component::audio::{BufferData, Buffer, BufferMut, channels_mut};
/// let mut buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0]);
/// for channel in channels_mut(&mut buffer) {
///    for sample in channel {
///      *sample *= 2.0;
///    }
/// }
/// assert_eq!(buffer.channel(0), [2.0, 4.0, 6.0]);
/// ```
pub fn channels_mut<B: BufferMut>(buffer: &mut B) -> impl Iterator<Item = &mut [f32]> {
    (0..buffer.num_channels()).map(move |channel| unsafe {
        std::slice::from_raw_parts_mut(
            buffer.channel_mut(channel).as_mut_ptr(),
            buffer.num_frames(),
        )
    })
}

/// A buffer of audio samples that owns its data.
///
/// This is a simple implementation of [`Buffer`] that owns its data on the heap.
/// It is useful for testing and as a simple way to create buffers.
///
/// # Examples
///
/// ```
/// # use conformal_component::audio::{BufferData, Buffer};
/// let buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0]);
/// assert_eq!(buffer.channel(0), [1.0, 2.0, 3.0]);
/// ```
#[derive(Debug, Clone)]
pub struct BufferData {
    channel_layout: ChannelLayout,
    num_frames: usize,
    data: Vec<f32>,
}

impl BufferData {
    /// Create a new buffer with the given channel layout and number of frames.
    ///
    /// The buffer will be filled with zeros.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::audio::{Buffer, BufferData, ChannelLayout};
    /// let buffer = BufferData::new(ChannelLayout::Mono, 3);
    /// assert_eq!(buffer.channel_layout(), ChannelLayout::Mono);
    /// assert_eq!(buffer.channel(0), [0.0, 0.0, 0.0]);
    /// ```
    #[must_use]
    pub fn new(channel_layout: ChannelLayout, num_frames: usize) -> Self {
        Self {
            channel_layout,
            num_frames,
            data: vec![0f32; channel_layout.num_channels() * num_frames],
        }
    }

    /// Create a new mono buffer with the given data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::audio::{Buffer, BufferData, ChannelLayout};
    /// let buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0]);
    /// assert_eq!(buffer.channel_layout(), ChannelLayout::Mono);
    /// assert_eq!(buffer.channel(0), [1.0, 2.0, 3.0]);
    /// ```
    #[must_use]
    pub fn new_mono(data: Vec<f32>) -> BufferData {
        Self {
            channel_layout: ChannelLayout::Mono,
            num_frames: data.len(),
            data,
        }
    }

    /// Create a new stereo buffer with the given data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::audio::{Buffer, BufferData, ChannelLayout, channels};
    /// let buffer = BufferData::new_stereo([1.0, 2.0], [3.0, 4.0]);
    /// assert_eq!(buffer.channel_layout(), ChannelLayout::Stereo);
    /// assert!(channels(&buffer).eq([[1.0, 2.0], [3.0, 4.0]]));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the length of `left` and `right` are not equal.
    ///
    /// ```should_panic
    /// # use conformal_component::audio::BufferData;
    /// let buffer = BufferData::new_stereo([1.0, 2.0], [3.0]);
    /// ```
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
