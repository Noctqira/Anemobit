use std::collections::VecDeque;

/// Fixed-capacity ring buffer; drops the oldest element when full.
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    capacity: usize,
    items: VecDeque<T>,
}

impl<T> RingBuffer<T> {
    /// `capacity` must be greater than 0.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity must be greater than 0");
        RingBuffer {
            capacity,
            items: VecDeque::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn push(&mut self, item: T) {
        if self.items.len() == self.capacity {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }

    pub fn last(&self) -> Option<&T> {
        self.items.back()
    }

    /// Oldest to newest.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.items.iter()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(buffer: &RingBuffer<i32>) -> Vec<i32> {
        buffer.iter().copied().collect()
    }

    #[test]
    fn keeps_insertion_order_below_capacity() {
        let mut buffer = RingBuffer::new(3);
        assert!(buffer.is_empty());
        buffer.push(1);
        buffer.push(2);
        assert_eq!(items(&buffer), [1, 2]);
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.last(), Some(&2));
    }

    #[test]
    fn drops_oldest_when_full() {
        let mut buffer = RingBuffer::new(3);
        for i in 1..=7 {
            buffer.push(i);
        }
        assert_eq!(items(&buffer), [5, 6, 7]);
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.last(), Some(&7));
    }

    #[test]
    fn clear_empties_the_buffer() {
        let mut buffer = RingBuffer::new(2);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        buffer.clear();
        assert!(buffer.is_empty());
        assert_eq!(buffer.last(), None);
        buffer.push(9);
        assert_eq!(items(&buffer), [9]);
    }

    #[test]
    #[should_panic(expected = "capacity")]
    fn zero_capacity_is_rejected() {
        RingBuffer::<i32>::new(0);
    }
}
