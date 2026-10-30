//! Per-node queue of state changes: `every` keeps them in order, `latest` keeps only the newest.

use std::collections::VecDeque;

/// How many undelivered changes a node may hold in `every` mode; the oldest are dropped past this.
const MAX_QUEUED: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// One pulse per change, in order (queued across ticks).
    #[default]
    Every,
    /// Only the newest change of a tick.
    Latest,
}

impl Mode {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "every" => Some(Self::Every),
            "latest" => Some(Self::Latest),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Every => "every",
            Self::Latest => "latest",
        }
    }
}

#[derive(Debug)]
pub struct ChangeQueue<T> {
    mode: Mode,
    items: VecDeque<T>,
}

impl<T> ChangeQueue<T> {
    pub fn new(mode: Mode) -> Self {
        Self { mode, items: VecDeque::new() }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Changing to `Latest` collapses what is already queued to the newest change.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        if mode == Mode::Latest {
            while self.items.len() > 1 {
                self.items.pop_front();
            }
        }
    }

    /// Adds a change; returns how many older changes were dropped because of the mode or the limit.
    pub fn push(&mut self, item: T) -> usize {
        let mut dropped = 0;
        if self.mode == Mode::Latest {
            dropped = self.items.len();
            self.items.clear();
        } else if self.items.len() >= MAX_QUEUED {
            self.items.pop_front();
            dropped = 1;
        }
        self.items.push_back(item);
        dropped
    }

    /// The change to emit in this tick (at most one per node per tick).
    pub fn pop(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.items.len()
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_keeps_order_one_per_pop() {
        let mut q = ChangeQueue::new(Mode::Every);
        for i in 1..=3 {
            assert_eq!(q.push(i), 0);
        }
        assert_eq!((q.pop(), q.pop(), q.pop(), q.pop()), (Some(1), Some(2), Some(3), None));
    }

    #[test]
    fn latest_keeps_the_newest_only() {
        let mut q = ChangeQueue::new(Mode::Latest);
        assert_eq!(q.push(1), 0);
        assert_eq!(q.push(2), 1);
        assert_eq!(q.push(3), 1);
        assert_eq!((q.pop(), q.pop()), (Some(3), None));
    }

    #[test]
    fn switching_to_latest_collapses_the_queue() {
        let mut q = ChangeQueue::new(Mode::Every);
        q.push(1);
        q.push(2);
        q.set_mode(Mode::Latest);
        assert_eq!(q.len(), 1);
        assert_eq!(q.pop(), Some(2));
    }

    #[test]
    fn every_is_bounded() {
        let mut q = ChangeQueue::new(Mode::Every);
        let dropped: usize = (0..MAX_QUEUED + 5).map(|i| q.push(i)).sum();
        assert_eq!(dropped, 5);
        assert_eq!(q.len(), MAX_QUEUED);
        assert_eq!(q.pop(), Some(5));
    }

    #[test]
    fn mode_parses_names() {
        assert_eq!(Mode::parse(" Latest "), Some(Mode::Latest));
        assert_eq!(Mode::parse("every"), Some(Mode::Every));
        assert_eq!(Mode::parse("x"), None);
    }
}
