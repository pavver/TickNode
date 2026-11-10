//! Bounded per-node FIFO of changes, delivered one per tick.

use std::collections::VecDeque;

const MAX_QUEUED: usize = 1000;

#[derive(Debug)]
pub struct ChangeQueue<T> {
    items: VecDeque<T>,
}

impl<T> ChangeQueue<T> {
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
        }
    }

    /// Adds a change; returns the number of oldest changes dropped past the limit.
    pub fn push(&mut self, item: T) -> usize {
        let dropped = usize::from(self.items.len() >= MAX_QUEUED);
        if dropped > 0 {
            self.items.pop_front();
        }
        self.items.push_back(item);
        dropped
    }

    pub fn pop(&mut self) -> Option<T> {
        self.items.pop_front()
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_order_one_per_pop() {
        let mut q = ChangeQueue::new();
        for i in 1..=3 {
            assert_eq!(q.push(i), 0);
        }
        assert_eq!(
            (q.pop(), q.pop(), q.pop(), q.pop()),
            (Some(1), Some(2), Some(3), None)
        );
    }

    #[test]
    fn bounds_and_clears_the_queue() {
        let mut q = ChangeQueue::new();
        let dropped: usize = (0..MAX_QUEUED + 5).map(|i| q.push(i)).sum();
        assert_eq!(dropped, 5);
        assert_eq!(q.pop(), Some(5));
        q.clear();
        assert_eq!(q.pop(), None);
    }
}
