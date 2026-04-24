use std::rc::Rc;

use gettextrs::gettext;
use gtk4::{gio, glib, pango, prelude::*};

use crate::project_tree_model::{ProjectTreeItem, ProjectTreeModel};

#[derive(Clone, Debug)]
pub(crate) enum ProjectTreeActivation {
    RegularFile(gio::File),
    Symlink(gio::File),
}

pub(crate) struct ProjectTree {
    model: ProjectTreeModel,
    selection: gtk4::SingleSelection,
    scroller: gtk4::ScrolledWindow,
}

impl ProjectTree {
    #[must_use]
    pub(crate) fn new(on_activate: Rc<dyn Fn(ProjectTreeActivation)>) -> Self {
        let model = ProjectTreeModel::new();
        let selection = gtk4::SingleSelection::new(Some(model.model().clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);

        let factory = create_factory();
        let list_view = create_list_view(&selection, &model, factory, on_activate);
        let scroller = create_scroller(&list_view);

        Self {
            model,
            selection,
            scroller,
        }
    }

    #[must_use]
    pub(crate) fn widget(&self) -> &gtk4::ScrolledWindow {
        &self.scroller
    }

    #[must_use]
    pub(crate) fn model(&self) -> &ProjectTreeModel {
        &self.model
    }

    #[must_use]
    pub(crate) fn selection(&self) -> &gtk4::SingleSelection {
        &self.selection
    }

    pub(crate) fn clear_selection(&self) {
        self.selection.set_selected(gtk4::INVALID_LIST_POSITION);
    }

    pub(crate) fn set_root(&self, root: Option<gio::File>) {
        self.model.set_root(root);
        self.clear_selection();
    }

    #[cfg(test)]
    pub(crate) fn expand_entry_for_tests(&self, name: &str) -> bool {
        self.model.expand_entry_for_tests(name)
    }

    #[cfg(test)]
    pub(crate) fn selected_uri_for_tests(&self) -> Option<String> {
        let position = self.selection.selected();
        if position == gtk4::INVALID_LIST_POSITION {
            return None;
        }
        let row = self.model.model().row(position)?;
        let item = row.item()?;
        let boxed = item.downcast::<glib::BoxedAnyObject>().ok()?;
        let borrowed = boxed.try_borrow::<ProjectTreeItem>().ok()?;
        match &*borrowed {
            ProjectTreeItem::Entry(entry) => Some(entry.uri.clone()),
            ProjectTreeItem::Loading | ProjectTreeItem::Error(_) => None,
        }
    }
}

fn create_factory() -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(setup_project_tree_row);
    factory.connect_bind(bind_project_tree_row);
    factory.connect_unbind(unbind_project_tree_row);
    factory
}

fn setup_project_tree_row(_: &gtk4::SignalListItemFactory, object: &glib::Object) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let expander = gtk4::TreeExpander::new();
    expander.set_indent_for_depth(true);

    let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    row_box.set_margin_start(6);
    row_box.set_margin_end(6);
    row_box.set_margin_top(3);
    row_box.set_margin_bottom(3);

    let icon = gtk4::Image::new();
    icon.set_icon_name(Some("text-x-generic-symbolic"));
    icon.set_pixel_size(16);

    let spinner = gtk4::Spinner::new();
    spinner.set_visible(false);
    spinner.set_spinning(true);

    let label = gtk4::Label::new(None);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_ellipsize(pango::EllipsizeMode::End);

    row_box.append(&icon);
    row_box.append(&spinner);
    row_box.append(&label);

    expander.set_child(Some(&row_box));
    list_item.set_child(Some(&expander));
}

fn bind_project_tree_row(_: &gtk4::SignalListItemFactory, object: &glib::Object) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let Some(row) = list_item
        .item()
        .and_then(|item| item.downcast::<gtk4::TreeListRow>().ok())
    else {
        return;
    };
    let Some(expander) = list_item
        .child()
        .and_then(|child| child.downcast::<gtk4::TreeExpander>().ok())
    else {
        return;
    };
    expander.set_list_row(Some(&row));

    let Some((icon, spinner, label)) = row_widgets(&expander) else {
        return;
    };
    let Some(row_item) = row.item() else {
        return;
    };
    let Ok(boxed) = row_item.downcast::<glib::BoxedAnyObject>() else {
        return;
    };
    let Ok(borrowed) = boxed.try_borrow::<ProjectTreeItem>() else {
        return;
    };

    spinner.set_visible(false);
    match &*borrowed {
        ProjectTreeItem::Loading => {
            icon.set_icon_name(Some("folder-symbolic"));
            label.set_label(&gettext("Loading..."));
            spinner.set_visible(true);
        }
        ProjectTreeItem::Error(message) => {
            icon.set_icon_name(Some("dialog-error-symbolic"));
            label.set_label(message);
        }
        ProjectTreeItem::Entry(entry) => {
            let icon_name = match entry.file_type {
                gio::FileType::Directory => "folder-symbolic",
                _ => "text-x-generic-symbolic",
            };
            icon.set_icon_name(Some(icon_name));
            label.set_label(&entry.display_name);
        }
    }
}

fn row_widgets(expander: &gtk4::TreeExpander) -> Option<(gtk4::Image, gtk4::Spinner, gtk4::Label)> {
    let row_box = expander.child()?.downcast::<gtk4::Box>().ok()?;
    let icon = row_box.first_child()?.downcast::<gtk4::Image>().ok()?;
    let spinner = icon.next_sibling()?.downcast::<gtk4::Spinner>().ok()?;
    let label = spinner.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    Some((icon, spinner, label))
}

fn unbind_project_tree_row(_: &gtk4::SignalListItemFactory, object: &glib::Object) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let Some(expander) = list_item
        .child()
        .and_then(|child| child.downcast::<gtk4::TreeExpander>().ok())
    else {
        return;
    };
    expander.set_list_row(None);
}

fn create_list_view(
    selection: &gtk4::SingleSelection,
    model: &ProjectTreeModel,
    factory: gtk4::SignalListItemFactory,
    on_activate: Rc<dyn Fn(ProjectTreeActivation)>,
) -> gtk4::ListView {
    let list_view = gtk4::ListView::new(Some(selection.clone()), Some(factory));
    list_view.set_single_click_activate(false);
    list_view.set_enable_rubberband(false);

    let model_for_activation = model.model().clone();
    list_view.connect_activate(move |_, position| {
        activate_tree_position(&model_for_activation, position, &on_activate);
    });
    list_view
}

fn activate_tree_position(
    model: &gtk4::TreeListModel,
    position: u32,
    on_activate: &Rc<dyn Fn(ProjectTreeActivation)>,
) {
    let Some(row) = model.row(position) else {
        return;
    };
    let Some(row_item) = row.item() else {
        return;
    };
    let Ok(boxed) = row_item.downcast::<glib::BoxedAnyObject>() else {
        return;
    };
    let Ok(borrowed) = boxed.try_borrow::<ProjectTreeItem>() else {
        return;
    };
    let ProjectTreeItem::Entry(entry) = &*borrowed else {
        return;
    };

    match entry.file_type {
        gio::FileType::Directory if row.is_expandable() => {
            row.set_expanded(!row.is_expanded());
        }
        gio::FileType::SymbolicLink => {
            on_activate(ProjectTreeActivation::Symlink(entry.file.clone()));
        }
        gio::FileType::Regular => {
            on_activate(ProjectTreeActivation::RegularFile(entry.file.clone()));
        }
        _ => {}
    }
}

fn create_scroller(list_view: &gtk4::ListView) -> gtk4::ScrolledWindow {
    let scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .child(list_view)
        .build();
    scroller.set_hexpand(true);
    scroller.set_vexpand(true);
    scroller
}
