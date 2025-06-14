use crate::gp_type::GpType;

pub struct StreamBuffer {
    len: usize,
    buf: Box<[u8]>,
    pos: usize,
}

impl StreamBuffer {
    pub fn new<T: AsRef<[u8]>>(initial: T) -> Self {
        let initial = initial.as_ref();
        let mut buf = Vec::with_capacity(initial.len());
        buf.extend_from_slice(initial);

        StreamBuffer {
            len: initial.len(),
            buf: buf.into_boxed_slice(),
            pos: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        StreamBuffer {
            len: capacity,
            buf: vec![0; capacity].into_boxed_slice(),
            pos: 0,
        }
    }

    fn check_size(&mut self, required_size: usize) -> bool {
        // Checks if the buffer is large enough for the required size
        // If not, it resizes the buffer
        if required_size <= self.buf.len() {
            return false;
        }

        // Calculate new capacity (double the current size or required size, whichever is larger)
        let new_capacity = std::cmp::max(self.buf.len() * 2, required_size);

        // Create a new buffer with the new capacity
        let mut new_buf = Vec::with_capacity(new_capacity);
        new_buf.extend_from_slice(&self.buf);
        new_buf.resize(new_capacity, 0);

        // Replace the old buffer with the new one
        self.buf = new_buf.into_boxed_slice();

        true
    }

    pub fn ensure_capacity(&mut self, additional: usize) {
        let required_size = self.pos + additional;
        self.check_size(required_size);
    }

    pub fn read_byte(&mut self) -> u8 {
        if self.pos >= self.len {
            panic!("Attempted to read past the end of the buffer");
        }

        let byte = self.buf[self.pos];
        self.pos += 1;
        byte
    }

    // Safe version that returns Option<u8>
    pub fn try_read_byte(&mut self) -> Option<u8> {
        if self.pos >= self.len {
            return None;
        }

        let byte = self.buf[self.pos];
        self.pos += 1;
        Some(byte)
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.ensure_capacity(1);

        self.buf[self.pos] = byte;
        self.pos += 1;

        if self.pos > self.len {
            self.len = self.pos;
        }
    }
    
    pub fn write_gp_type(&mut self, gp_type: GpType) {
        self.write_byte(gp_type.into())
    }

    pub fn write(&mut self, data: &[u8]) {
        self.ensure_capacity(data.len());

        for i in 0..data.len() {
            self.buf[self.pos + i] = data[i];
        }

        self.pos += data.len();

        if self.pos > self.len {
            self.len = self.pos;
        }
    }

    pub fn read(&mut self, count: usize) -> Vec<u8> {
        let available = std::cmp::min(count, self.len - self.pos);
        let mut result = Vec::with_capacity(available);

        for i in 0..available {
            result.push(self.buf[self.pos + i]);
        }

        self.pos += available;
        result
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn length(&self) -> usize {
        self.len
    }

    pub fn reset_position(&mut self) {
        self.pos = 0;
    }

    pub fn remaining(&self) -> usize {
        self.len.saturating_sub(self.pos)
    }
    
    pub fn seek(&mut self, position: usize) {
        self.pos = position;
    }
    
    pub fn get_buffer(&self) -> &[u8] {
        &self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let initial = [1, 2, 3, 4, 5];
        let buffer = StreamBuffer::new(&initial);

        assert_eq!(buffer.length(), 5);
        assert_eq!(buffer.position(), 0);
    }

    #[test]
    fn test_with_capacity() {
        let buffer = StreamBuffer::with_capacity(10);

        assert_eq!(buffer.length(), 10);
        assert_eq!(buffer.position(), 0);
        assert_eq!(buffer.buf.len(), 10);
    }

    #[test]
    fn test_read_write_byte() {
        let mut buffer = StreamBuffer::with_capacity(5);

        buffer.write_byte(42);
        buffer.write_byte(43);
        buffer.reset_position();

        assert_eq!(buffer.read_byte(), 42);
        assert_eq!(buffer.read_byte(), 43);
        // Reading past the end would panic, so we don't test that here
    }

    #[test]
    fn test_try_read_write_byte() {
        let mut buffer = StreamBuffer::with_capacity(2);

        buffer.write_byte(42);
        buffer.write_byte(43);
        buffer.reset_position();

        assert_eq!(buffer.try_read_byte(), Some(42));
        assert_eq!(buffer.try_read_byte(), Some(43));
        assert_eq!(buffer.try_read_byte(), None); // No more data
    }

    #[test]
    fn test_read_write_multiple() {
        let mut buffer = StreamBuffer::with_capacity(5);
        let data = [1, 2, 3, 4, 5];

        buffer.write(&data);
        buffer.reset_position();

        let read_data = buffer.read(5);
        assert_eq!(read_data, data);
    }

    #[test]
    fn test_auto_resize() {
        let mut buffer = StreamBuffer::with_capacity(2);

        // Write more data than the initial capacity
        let data = [1, 2, 3, 4, 5];
        buffer.write(&data);

        // Buffer should have resized
        assert!(buffer.buf.len() >= 5);
        assert_eq!(buffer.length(), 5);

        buffer.reset_position();
        let read_data = buffer.read(5);
        assert_eq!(read_data, data);
    }

    #[test]
    fn test_position_and_remaining() {
        let mut buffer = StreamBuffer::new(&[1, 2, 3, 4, 5]);

        assert_eq!(buffer.position(), 0);
        assert_eq!(buffer.remaining(), 5);

        buffer.read_byte();
        buffer.read_byte();

        assert_eq!(buffer.position(), 2);
        assert_eq!(buffer.remaining(), 3);
    }
}
