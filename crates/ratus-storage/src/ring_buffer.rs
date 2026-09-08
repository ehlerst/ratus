//! High-performance fixed-capacity circular ring buffer.

use serde::{Deserialize, Serialize};

/// A bounded, fixed-capacity circular ring buffer that overwrites oldest elements when full.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingBuffer<T> {
    data: Vec<T>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl<T: Clone> RingBuffer<T> {
    /// Create a new ring buffer with a fixed maximum capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity must be > 0");
        Self {
            data: Vec::with_capacity(capacity),
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Push an item into the ring buffer, overwriting the oldest element if full.
    pub fn push(&mut self, item: T) {
        if self.len < self.capacity {
            self.data.push(item);
            self.len += 1;
        } else {
            self.data[self.head] = item;
            self.head = (self.head + 1) % self.capacity;
        }
    }

    /// Number of elements currently in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Maximum capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all elements from the buffer.
    pub fn clear(&mut self) {
        self.data.clear();
        self.head = 0;
        self.len = 0;
    }

    /// Retrieve elements ordered from oldest to newest.
    pub fn to_vec(&self) -> Vec<T> {
        if self.len < self.capacity {
            self.data.clone()
        } else {
            let mut result = Vec::with_capacity(self.len);
            result.extend_from_slice(&self.data[self.head..]);
            result.extend_from_slice(&self.data[..self.head]);
            result
        }
    }

    /// Get reference to newest element, if any.
    pub fn latest(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else if self.len < self.capacity {
            self.data.last()
        } else {
            let idx = (self.head + self.capacity - 1) % self.capacity;
            Some(&self.data[idx])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_push_and_eviction() {
        let mut buf = RingBuffer::new(3);
        assert!(buf.is_empty());

        buf.push(1);
        buf.push(2);
        assert_eq!(buf.to_vec(), vec![1, 2]);

        buf.push(3);
        assert_eq!(buf.to_vec(), vec![1, 2, 3]);

        // Overwrite oldest (1)
        buf.push(4);
        assert_eq!(buf.to_vec(), vec![2, 3, 4]);

        // Overwrite oldest (2)
        buf.push(5);
        assert_eq!(buf.to_vec(), vec![3, 4, 5]);

        assert_eq!(buf.latest(), Some(&5));
    }
}
