#[derive(Debug)]
pub struct RingBuffer<const SIZE: usize> {
    /// Buffer of data.
    data: Option<u8>,
}


impl<const SIZE: usize> RingBuffer<SIZE> {
    pub const fn new() -> Self {
        Self {
            data: None
        }
    }

    /// Number of available elements to read.
    pub fn available(&self) -> usize {
        if self.data.is_some() {
            1
        } else {
            0
        }
    }

    /// Whether buffer is full.
    pub fn is_full(&self) -> bool {
        self.available() == 1
    }

    /// Append data to end of buffer.
    /// Returns `true` if successful.
    pub fn push_back(&mut self, data: u8) -> bool {
        if self.is_full() {
            return false;
        }
        self.data = Some(data);
        true
    }

    /// Pop data from start of buffer.
    pub fn pop_front(&mut self) -> Option<u8> {
        if self.available() == 0 {
            return None;
        }
        self.data.take()
    }
}
