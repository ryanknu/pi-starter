use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::ops::{Deref, DerefMut};
use crate::{EVENT_BUFFER_LEN, EVENT_SIZE};

pub trait ReadInputStream {
    fn read_events(&mut self, stream: impl Iterator<Item = InputEvent>) -> Result<bool, Box<dyn Error>>;
}

/// Represents a raw input event from the Linux evdev system.
pub struct InputEvent {
    pub r#type: u8,
    pub code: u8,
    pub value: i32,
}

/// Represents a generic input device
pub struct InputDevice<T: ReadInputStream> {
    /// Handle to file for the input device.
    file: File,
    /// Buffer for reading input data. Event structure is 8 bytes timestamp, 2 bytes type, 2 bytes
    /// code, and 4 bytes for value. 16 bytes per event with a 16 byte buffer.
    data: [u8; EVENT_SIZE * EVENT_BUFFER_LEN],
    /// ..
    device: T,
}

impl<T> InputDevice<T> where T: ReadInputStream + Default {
    /// Creates an Input device with the default implementation of the InputDevice.
    pub(crate) fn new(file: File) -> Self {
        Self {
            file,
            data: [0; EVENT_SIZE * EVENT_BUFFER_LEN],
            device: T::default(),
        }
    }

    /// Polls the input device for any new data. Returns true, false, or an error. Bool return types
    /// indicate whether or not there's anything to process on the user's end. An error return type
    /// indicates that the user should terminate the process, or gracefully handle the error.
    pub fn poll(&mut self) -> Result<bool, Box<dyn Error>> {
        // Read up to N events into buffer.
        let bytes_read = self.file.read(&mut self.data)?;
        if bytes_read == 0 {
            return Ok(false);
        }

        // Take a reference to the slice that only contains data read.
        let events = &self.data[..bytes_read];

        // Turn a simple array of bytes into an iterator over well-formed events.
        // TODO: Check to make sure that creating InputEvent structs from this is not slow.
        let events = events.chunks(EVENT_SIZE).map(|raw_event| InputEvent {
            r#type: raw_event[8],
            code: raw_event[10],
            value: i32::from_le_bytes([raw_event[12], raw_event[13], raw_event[14], raw_event[15]]),
        });

        // Pass to the device abstraction and return the result.
        Ok(self.device.read_events(events)?)
    }
}

/// Implementing Deref allows us to use the inner device as if it's fields are part of the base
/// struct.
impl<T: ReadInputStream> Deref for InputDevice<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

/// See: `deref`
impl<T: ReadInputStream> DerefMut for InputDevice<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.device
    }
}
