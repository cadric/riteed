use super::Window;

impl Window {
    pub(crate) fn project_monitor_count_for_tests(&self) -> usize {
        self.project.project_monitor_count_for_tests()
    }

    pub(crate) fn trigger_project_auto_refresh_for_tests(&self) {
        self.project.trigger_project_auto_refresh_for_tests();
    }

    pub(crate) fn expand_project_tree_entry_for_tests(&self, name: &str) -> bool {
        self.project.expand_tree_entry_for_tests(name)
    }

    pub(crate) fn selected_project_tree_uri_for_tests(&self) -> Option<String> {
        self.project.selected_tree_uri_for_tests()
    }
}
