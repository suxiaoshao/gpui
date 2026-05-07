use gpui::SharedString;
use std::hash::Hash;

pub(crate) trait PickerOption: Clone + 'static {
    type Key: Clone + Eq + Hash + 'static;

    fn key(&self) -> Self::Key;
    fn label(&self) -> SharedString;
    fn description(&self) -> Option<SharedString> {
        None
    }

    fn matches(&self, query: &str) -> bool {
        if query.trim().is_empty() {
            return true;
        }
        let query = query.to_lowercase();
        self.label().to_lowercase().contains(&query)
            || self
                .description()
                .is_some_and(|description| description.to_lowercase().contains(&query))
    }
}
