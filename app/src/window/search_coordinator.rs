use std::rc::Rc;

use gtk4::gio;

use crate::editor_search::{EditorSearch, ProjectSearchRequest};
use crate::find_in_files::FindInFilesController;
use crate::sidebar_host::SidebarHost;

pub(super) fn install(
    editor_search: &Rc<EditorSearch>,
    find_in_files: Rc<FindInFilesController>,
    sidebar_host: Rc<SidebarHost>,
    root_provider: Rc<dyn Fn() -> Option<gio::File>>,
) {
    let dispatch: Rc<dyn Fn(ProjectSearchRequest)> = Rc::new(move |request| match request {
        ProjectSearchRequest::Query { query, match_case } => {
            sidebar_host.set_search_results_visible(true);
            sidebar_host.select_search_results();
            if root_provider().is_none() {
                find_in_files.show_root_missing();
            } else if query.is_empty() {
                find_in_files.clear();
            } else {
                find_in_files.set_query(&query, match_case);
            }
        }
    });
    editor_search.set_project_search_dispatch(dispatch);
}
