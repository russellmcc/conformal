use vst3::{
    ComRef,
    Steinberg::{
        IBStream,
        IBStream_::IStreamSeekMode_::kIBSeekSet,
        IBStreamTrait,
    },
};

pub struct StreamWrite<'a> {
    buffer: ComRef<'a, IBStream>,
}

impl<'a> StreamWrite<'a> {
    /// WARNING - do not modify or read from the buffer while a `StreamWrite` is active.
    pub fn new(buffer: ComRef<'a, IBStream>) -> Self {
        Self { buffer }
    }
}

impl std::io::Write for StreamWrite<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        unsafe {
            let mut num_written = 0;
            match self.buffer.write(
                buf.as_ptr() as *mut std::ffi::c_void,
                buf.len().try_into().unwrap(),
                &raw mut num_written,
            ) {
                vst3::Steinberg::kResultOk => Ok(num_written.try_into().unwrap()),
                _ => Err(std::io::Error::other("VST3 write error")),
            }
        }
    }

    /// We treat the vst3 stream as a sink, so no flushing is required.
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Result::Ok(())
    }
}

#[derive(Clone)]
pub struct StreamRead<'a> {
    buffer: ComRef<'a, IBStream>,
}

impl<'a> StreamRead<'a> {
    /// WARNING - do not modify or read from the buffer while a `StreamRead` is active.
    pub fn new(buffer: ComRef<'a, IBStream>) -> Self {
        Self { buffer }
    }

    pub fn seek_to_start(&self) -> std::io::Result<()> {
        unsafe {
            let result = self.buffer.seek(0, kIBSeekSet as i32, std::ptr::null_mut());
            if result == vst3::Steinberg::kResultOk {
                Ok(())
            } else {
                Err(std::io::Error::other("VST3 seek error"))
            }
        }
    }
}

impl std::io::Read for StreamRead<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        unsafe {
            let mut num_read = 0;
            match self.buffer.read(
                buf.as_mut_ptr().cast::<std::ffi::c_void>(),
                buf.len().try_into().unwrap(),
                &raw mut num_read,
            ) {
                vst3::Steinberg::kResultOk => Ok(num_read.try_into().unwrap()),
                _ => Err(std::io::Error::other("VST3 read error")),
            }
        }
    }
}
