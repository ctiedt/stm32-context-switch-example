#[derive(Debug)]
pub struct FIFO<T: Copy, const SIZE: usize> {
    /// Buffer of data.
    data: [T; SIZE],
    /// Start of data.
    read_head: usize,
    /// End of data.
    write_head: usize,
    /// When `begin == end`, buffer is either empty or completely full.
    count: usize,
}


impl<T: Copy, const SIZE: usize> FIFO<T, SIZE> {
    pub const fn new_with(default: T) -> Self {
        Self {
            data: [default; SIZE],
            read_head: 0,
            write_head: 0,
            count: 0,
        }
    }

    /// Number of available elements to read.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Number of free slots.
    pub fn free_space(&self) -> usize {
        SIZE - self.len()
    }

    /// Whether buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Whether buffer is full.
    pub fn is_full(&self) -> bool { self.count == SIZE }

    /// Append data to end of buffer.
    /// Returns `true` if successful.
    pub fn push_back(&mut self, data: T) -> bool {
        if self.is_full() {
            return false;
        }
        self.data[self.write_head] = data;
        self.write_head = (self.write_head + 1) % SIZE;
        self.count += 1;
        true
    }

    /// Tries to append an entire array of elements.
    /// Returns [`Ok(len(data)`] if all elements were copied to the buffer and [`Err(count)`] where
    /// `count` is the number of appended elements when there was not enough space.
    pub fn append(&mut self, data: &[T]) -> Result<usize, usize> {
        let mut appended = 0;
        for element in data {
            if !self.push_back(*element) {
                return Err(appended);
            }
            appended += 1;
        }
        Ok(appended)
    }

    /// Pop data from start of buffer.
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let data = self.data[self.read_head];
        self.read_head = (self.read_head + 1) % SIZE;
        self.count -= 1;
        Some(data)
    }
}
