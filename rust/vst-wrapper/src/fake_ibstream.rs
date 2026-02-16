use std::cell::RefCell;

use vst3::{
    Class,
    Steinberg::{IBStream, IBStreamTrait},
};

use crate::i32_to_enum;

#[derive(Default)]
pub struct Stream {
    data: RefCell<Vec<u8>>,
    head: RefCell<usize>,
}

impl Stream {
    pub fn new<I: IntoIterator<Item = u8>>(data: I) -> Self {
        Self {
            data: RefCell::new(data.into_iter().collect()),
            head: RefCell::new(0),
        }
    }

    pub fn data(&self) -> Vec<u8> {
        self.data.borrow().clone()
    }
}

impl IBStreamTrait for Stream {
    unsafe fn read(
        &self,
        buffer: *mut std::ffi::c_void,
        num_bytes: vst3::Steinberg::int32,
        num_bytes_read_out: *mut vst3::Steinberg::int32,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            // Read the bytes into the passed-in buffer
            let num_bytes = num_bytes as usize;
            let num_bytes_read =
                std::cmp::min(num_bytes, self.data.borrow().len() - *self.head.borrow());
            std::ptr::copy_nonoverlapping(
                self.data.borrow_mut().as_ptr().add(*self.head.borrow()),
                buffer as *mut u8,
                num_bytes_read,
            );
            self.head.replace_with(|x| *x + num_bytes_read);
            if !num_bytes_read_out.is_null() {
                *num_bytes_read_out = num_bytes_read as i32;
            }
            vst3::Steinberg::kResultOk
        }
    }

    unsafe fn write(
        &self,
        buffer: *mut std::ffi::c_void,
        num_bytes: vst3::Steinberg::int32,
        num_bytes_written: *mut vst3::Steinberg::int32,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            // Write the bytes from the passed-in buffer, expanding if necessary.
            let num_bytes = num_bytes as usize;
            if *self.head.borrow() + num_bytes > self.data.borrow().len() {
                self.data
                    .borrow_mut()
                    .resize(*self.head.borrow() + num_bytes, 0);
            }
            std::ptr::copy_nonoverlapping(
                buffer as *mut u8,
                self.data.borrow_mut().as_mut_ptr().add(*self.head.borrow()),
                num_bytes,
            );
            self.head.replace_with(|x| *x + num_bytes);
            if !num_bytes_written.is_null() {
                *num_bytes_written = num_bytes as i32;
            }
            vst3::Steinberg::kResultOk
        }
    }

    unsafe fn seek(
        &self,
        pos: vst3::Steinberg::int64,
        mode: vst3::Steinberg::int32,
        result: *mut vst3::Steinberg::int64,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if let Some(next_head) = match i32_to_enum(mode) {
                Ok(vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekCur) => {
                    let saturated = if pos < 0 {
                        self.head.borrow().saturating_sub((-pos) as usize)
                    } else {
                        self.head.borrow().saturating_add(pos as usize)
                    };
                    Some(if saturated > self.data.borrow().len() {
                        self.data.borrow().len()
                    } else {
                        saturated
                    })
                }
                Ok(vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet) => Some(
                    pos.clamp(0, self.data.borrow().len().try_into().unwrap())
                        .try_into()
                        .unwrap(),
                ),
                Ok(vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekEnd) => Some(
                    self.data
                        .borrow()
                        .len()
                        .saturating_sub(pos.abs().try_into().unwrap()),
                ),
                _ => None,
            } {
                self.head.replace(next_head);
                if !result.is_null() {
                    *result = next_head.try_into().unwrap();
                }
                vst3::Steinberg::kResultOk
            } else {
                vst3::Steinberg::kInvalidArgument
            }
        }
    }

    unsafe fn tell(&self, pos: *mut vst3::Steinberg::int64) -> vst3::Steinberg::tresult {
        unsafe {
            if !pos.is_null() {
                *pos = *self.head.borrow() as i64;
            }
            vst3::Steinberg::kResultOk
        }
    }
}

impl Class for Stream {
    type Interfaces = (IBStream,);
}
