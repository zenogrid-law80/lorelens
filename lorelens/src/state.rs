use std::collections::HashSet;

#[derive(Default)]
pub(super) struct SelectionState {
    pub current: Option<String>,
    pub paths: HashSet<String>,
    pub anchor: Option<String>,
}

impl SelectionState {
    pub fn select(&mut self, path: String) {
        self.current = Some(path.clone());
        self.paths.clear();
        self.paths.insert(path.clone());
        self.anchor = Some(path);
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Default)]
pub(super) struct PreviewState {
    generation: u64,
    loading: bool,
}

impl PreviewState {
    pub fn begin(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.loading = true;
        self.generation
    }

    pub fn accepts(&self, generation: u64) -> bool {
        self.loading && self.generation == generation
    }

    pub fn finish(&mut self) {
        self.loading = false;
    }
    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.loading = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_previews_are_discarded() {
        let mut state = PreviewState::default();
        let first = state.begin();
        let second = state.begin();
        assert!(!state.accepts(first));
        assert!(state.accepts(second));
        state.invalidate();
        assert!(!state.accepts(second));
        let third = state.begin();
        state.finish();
        assert!(!state.accepts(third));
    }
}
