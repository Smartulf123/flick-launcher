//! The registered search modes, in Tab order.

use crate::mode::SearchMode;

#[derive(Default)]
pub struct ModeRegistry {
    modes: Vec<Box<dyn SearchMode>>,
}

impl ModeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a mode after the ones already registered.
    pub fn register(&mut self, mode: Box<dyn SearchMode>) {
        self.modes.push(mode);
    }

    pub fn len(&self) -> usize {
        self.modes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modes.is_empty()
    }

    /// The mode at `index`. Panics if `index >= len()`.
    pub fn mode(&self, index: usize) -> &dyn SearchMode {
        self.modes[index].as_ref()
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.modes.iter().position(|m| m.id() == id)
    }

    pub fn next(&self, index: usize) -> usize {
        (index + 1) % self.len()
    }

    pub fn prev(&self, index: usize) -> usize {
        (index + self.len() - 1) % self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::ResultItem;

    struct Dummy(&'static str);

    impl SearchMode for Dummy {
        fn id(&self) -> &str {
            self.0
        }
        fn name(&self) -> &str {
            self.0
        }
        fn query(&self, _text: &str, _max_results: usize) -> Vec<ResultItem> {
            Vec::new()
        }
    }

    fn registry() -> ModeRegistry {
        let mut r = ModeRegistry::new();
        r.register(Box::new(Dummy("google")));
        r.register(Box::new(Dummy("youtube")));
        r.register(Box::new(Dummy("files")));
        r
    }

    #[test]
    fn modes_keep_their_order() {
        let r = registry();
        assert_eq!(r.len(), 3);
        assert_eq!(r.mode(1).id(), "youtube");
        assert_eq!(r.index_of("files"), Some(2));
        assert_eq!(r.index_of("bing"), None);
    }

    #[test]
    fn cycling_wraps_both_ways() {
        let r = registry();
        assert_eq!((r.next(0), r.next(1), r.next(2)), (1, 2, 0));
        assert_eq!((r.prev(0), r.prev(2)), (2, 1));
    }
}
