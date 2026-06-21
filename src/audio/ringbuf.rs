use std::sync::atomic::{AtomicUsize, Ordering};

/// Lock-free single-producer single-consumer ring buffer for audio samples.
pub struct AudioRingBuffer {
    buffer: Vec<f32>,
    capacity: usize,
    /// Write index (producer)
    write_pos: AtomicUsize,
    /// Read index (consumer)
    read_pos: AtomicUsize,
}

impl AudioRingBuffer {
    /// Create a new ring buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        // Round up to power of 2 for efficient masking
        let cap = capacity.next_power_of_two();
        Self {
            buffer: vec![0.0f32; cap],
            capacity: cap,
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
        }
    }

    /// Returns the mask for index wrapping.
    #[inline]
    fn mask(&self) -> usize {
        self.capacity - 1
    }

    /// Push a single sample into the ring buffer.
    /// Returns `Ok(())` on success, or `Err(())` if the buffer is full.
    #[inline]
    pub fn push(&self, sample: f32) -> Result<(), ()> {
        let write = self.write_pos.load(Ordering::Acquire);
        let read = self.read_pos.load(Ordering::Relaxed);
        let len = write.wrapping_sub(read);

        if len >= self.capacity {
            return Err(()); // Full
        }

        // SAFETY: We have exclusive access to the write index (single producer)
        unsafe {
            let ptr = self.buffer.as_ptr().add(write & self.mask()) as *mut f32;
            *ptr = sample;
        }
        self.write_pos.store(write.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Push a slice of samples into the ring buffer.
    /// Returns the number of samples successfully pushed.
    pub fn push_slice(&self, samples: &[f32]) -> usize {
        let mut pushed = 0;
        for &sample in samples {
            if self.push(sample).is_ok() {
                pushed += 1;
            } else {
                break;
            }
        }
        pushed
    }

    /// Pop a single sample from the ring buffer.
    #[inline]
    pub fn pop(&self) -> Option<f32> {
        let read = self.read_pos.load(Ordering::Acquire);
        let write = self.write_pos.load(Ordering::Relaxed);
        let len = write.wrapping_sub(read);

        if len == 0 {
            return None; // Empty
        }

        // SAFETY: We have exclusive access to the read index
        let sample = unsafe {
            *self.buffer.as_ptr().add(read & self.mask())
        };
        self.read_pos.store(read.wrapping_add(1), Ordering::Release);
        Some(sample)
    }

    /// Pop up to `max` samples from the ring buffer into `output`.
    /// Returns the number of samples popped.
    pub fn pop_slice(&self, output: &mut [f32]) -> usize {
        let mut popped = 0;
        for out in output.iter_mut() {
            if let Some(sample) = self.pop() {
                *out = sample;
                popped += 1;
            } else {
                break;
            }
        }
        popped
    }

    /// Read samples without removing them.
    pub fn peek_slice(&self, output: &mut [f32]) -> usize {
        let read = self.read_pos.load(Ordering::Acquire);
        let write = self.write_pos.load(Ordering::Relaxed);
        let len = write.wrapping_sub(read);
        let to_read = output.len().min(len);

        for i in 0..to_read {
            let idx = read.wrapping_add(i) & self.mask();
            unsafe {
                output[i] = *self.buffer.as_ptr().add(idx);
            }
        }
        to_read
    }

    /// Return the number of samples available for reading.
    #[inline]
    pub fn len(&self) -> usize {
        let write = self.write_pos.load(Ordering::Acquire);
        let read = self.read_pos.load(Ordering::Relaxed);
        write.wrapping_sub(read)
    }

    /// Return the capacity of the ring buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Check if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the buffer by resetting both indices.
    pub fn clear(&self) {
        self.read_pos.store(self.write_pos.load(Ordering::Acquire), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop() {
        let buf = AudioRingBuffer::new(4);
        assert!(buf.is_empty());
        assert_eq!(buf.push(1.0), Ok(()));
        assert_eq!(buf.push(2.0), Ok(()));
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.pop(), Some(1.0));
        assert_eq!(buf.pop(), Some(2.0));
        assert!(buf.is_empty());
    }

    #[test]
    fn test_wraparound() {
        let buf = AudioRingBuffer::new(4);
        // Fill the buffer
        for i in 0..4 {
            assert!(buf.push(i as f32).is_ok());
        }
        assert!(buf.push(5.0).is_err()); // Full
        assert_eq!(buf.pop(), Some(0.0));
        assert_eq!(buf.pop(), Some(1.0));
        assert!(buf.push(5.0).is_ok()); // Should work now
        assert_eq!(buf.pop(), Some(2.0));
        assert_eq!(buf.pop(), Some(3.0));
        assert_eq!(buf.pop(), Some(5.0));
        assert!(buf.is_empty());
    }

    #[test]
    fn test_push_slice() {
        let buf = AudioRingBuffer::new(8);
        let data = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(buf.push_slice(&data), 4);
        assert_eq!(buf.len(), 4);

        let mut out = vec![0.0f32; 4];
        assert_eq!(buf.pop_slice(&mut out), 4);
        assert_eq!(out, data);
    }

    #[test]
    fn test_clear() {
        let buf = AudioRingBuffer::new(8);
        buf.push_slice(&[1.0, 2.0, 3.0]);
        assert_eq!(buf.len(), 3);
        buf.clear();
        assert!(buf.is_empty());
    }
}
