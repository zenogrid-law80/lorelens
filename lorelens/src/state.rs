use std::collections::HashSet;

#[derive(Default)]
pub(super) struct SelectionState {
    pub current: Option<String>,
    pub paths: HashSet<String>,
    pub anchor: Option<String>,
}

impl SelectionState {
    /// Context actions use the selection only when the clicked row belongs to it.
    pub fn context_paths(&self, clicked: &str, visible: &[String]) -> Vec<String> {
        if self.paths.contains(clicked) {
            visible.iter().filter(|path| self.paths.contains(*path)).cloned().collect()
        } else {
            visible.iter().filter(|path| path.as_str() == clicked).cloned().collect()
        }
    }

    pub fn select_all(&mut self, visible: &[String]) {
        self.paths = visible.iter().cloned().collect();
        self.current = self.current.take().filter(|p| self.paths.contains(p))
            .or_else(|| visible.first().cloned());
        self.anchor = self.current.clone();
    }

    pub fn click(&mut self, path: String, visible: &[String], additive: bool, range: bool) {
        if range {
            let start = self.anchor.as_ref().and_then(|p| visible.iter().position(|v| v == p));
            let end = visible.iter().position(|p| p == &path);
            if !additive { self.paths.clear(); }
            if let (Some(start), Some(end)) = (start, end) {
                self.paths.extend(visible[start.min(end)..=start.max(end)].iter().cloned());
            } else {
                self.paths.insert(path.clone());
                self.anchor = Some(path.clone());
            }
        } else {
            if !additive { self.paths.clear(); }
            if !self.paths.remove(&path) { self.paths.insert(path.clone()); }
            self.anchor = Some(path.clone());
        }
        self.current = if self.paths.contains(&path) { Some(path) }
            else { visible.iter().find(|p| self.paths.contains(*p)).cloned() };
    }

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
    fn context_action_uses_selected_pending_paths_or_the_clicked_row() {
        let pending = vec!["한글 file.txt".into(), "--option".into(), "other".into()];
        let mut state = SelectionState::default();
        state.paths = ["한글 file.txt", "--option", "outside-pending"].map(String::from).into();
        assert_eq!(state.context_paths("--option", &pending), pending[..2]);
        assert_eq!(state.context_paths("other", &pending), vec![String::from("other")]);
        assert!(state.context_paths("missing", &pending).is_empty());
    }

    #[test]
    fn large_selection_includes_paths_beyond_the_old_display_limit() {
        let visible: Vec<_> = (0..10_000).map(|i| format!("file-{i}")).collect();
        let mut state = SelectionState::default();
        state.select_all(&visible);
        assert_eq!(state.paths.len(), 10_000);
        assert_eq!(state.context_paths("file-9999", &visible), visible);
        state.click("file-9999".into(), &visible, true, false);
        assert_eq!(state.paths.len(), 9_999);
        assert!(!state.paths.contains("file-9999"));
        state.click("file-9999".into(), &visible, false, false);
        state.click("file-1999".into(), &visible, false, true);
        assert_eq!(state.paths.len(), 8_001);
        assert!(state.paths.contains("file-9999"));
        assert!(state.paths.contains("file-1999"));
    }

    #[test]
    fn multiple_selection_supports_toggle_and_ranges() {
        let visible: Vec<String> = ["a", "b", "c", "d"].map(String::from).to_vec();
        let mut state = SelectionState::default();
        state.click("b".into(), &visible, false, false);
        state.click("d".into(), &visible, true, false);
        assert_eq!(state.paths.len(), 2);
        state.click("d".into(), &visible, true, false);
        assert_eq!(state.current.as_deref(), Some("b"));
        state.click("b".into(), &visible, false, true);
        assert_eq!(state.paths, ["b", "c", "d"].map(String::from).into());
        state.click("a".into(), &visible, true, true);
        assert_eq!(state.paths.len(), 4);
        state.click("c".into(), &visible, false, false);
        assert_eq!(state.paths, [String::from("c")].into());
    }

    #[test]
    fn select_all_replaces_other_panel_selection_and_handles_empty_lists() {
        let mut state = SelectionState::default();
        state.select("outside".into());
        let visible = vec!["a".into(), "b".into()];
        state.select_all(&visible);
        assert_eq!(state.paths, visible.into_iter().collect());
        assert_eq!(state.current.as_deref(), Some("a"));
        state.select_all(&[]);
        assert!(state.paths.is_empty());
        assert!(state.current.is_none() && state.anchor.is_none());
    }

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
