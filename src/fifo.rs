#[derive(Debug)]
pub struct FIFO<T: Copy, const SIZE: usize> {
    /// Buffer of data.
    data: [T; SIZE],
    /// Start of data.
    read_head: usize,
    /// End of data.
    write_head: usize,
    /// When `begin == end`, buffer is either empty or completely full.
    is_empty: bool,
}


impl<T: Copy, const SIZE: usize> FIFO<T, SIZE> {
    pub const fn new_with(default: T) -> Self {
        Self {
            data: [default; SIZE],
            read_head: 0,
            write_head: 0,
            is_empty: true,
        }
    }

    /// Number of available elements to read.
    pub fn len(&self) -> usize {
        if self.is_empty {
            0
        } else if self.read_head < self.write_head {
            self.write_head - self.read_head
        } else {
            SIZE + self.write_head - self.read_head
        }
    }

    /// Whether buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.is_empty
    }

    /// Whether buffer is full.
    pub fn is_full(&self) -> bool { self.len() == SIZE }

    /// Append data to end of buffer.
    /// Returns `true` if successful.
    pub fn push_back(&mut self, data: T) -> bool {
        if self.is_full() {
            return false;
        }
        self.data[self.write_head] = data;
        self.write_head = (self.write_head + 1) % SIZE;
        self.is_empty = false;
        true
    }

    /// Pop data from start of buffer.
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let data = self.data[self.read_head];
        self.read_head = (self.read_head + 1) % SIZE;
        self.is_empty = self.read_head == self.write_head;
        Some(data)
    }
}
