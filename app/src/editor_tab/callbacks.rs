use std::rc::Rc;

use gtk4::{gdk, prelude::*};
use sourceview5::prelude::*;

use super::{EditorTab, VisibleBannerState};

impl EditorTab {
    pub(super) fn install_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.text_buffer.connect_changed(move |_| {
            if let Some(tab) = weak.upgrade()
                && !tab.state.borrow().ui.suppress_changes
            {
                tab.state.borrow_mut().mark_dirty_generation();
                tab.sync_presentation();
                tab.schedule_markdown_preview_update();
                tab.schedule_source_control_minimap_stale_check();
            }
        });

        let weak = Rc::downgrade(self);
        self.text_buffer.connect_modified_changed(move |_| {
            if let Some(tab) = weak.upgrade()
                && !tab.state.borrow().ui.suppress_changes
            {
                tab.sync_presentation();
                if !tab.pending_external_state().is_idle() {
                    tab.notify_external_state_change();
                }
                tab.schedule_source_control_minimap_stale_check();
            }
        });

        let weak = Rc::downgrade(self);
        self.text_buffer.connect_cursor_moved(move |_| {
            let Some(tab) = weak.upgrade() else {
                return;
            };
            let callback = tab.on_visual_change.borrow().clone();
            if let Some(callback) = callback {
                callback();
            }
        });

        let weak = Rc::downgrade(self);
        self.banner.connect_button_clicked(move |_| {
            let Some(tab) = weak.upgrade() else {
                return;
            };
            let callback = tab.on_external_action.borrow().clone();
            if let Some(callback) = callback {
                callback();
            }
        });

        let weak = Rc::downgrade(self);
        self.banner.connect_revealed_notify(move |banner| {
            if banner.is_revealed() {
                return;
            }
            let Some(tab) = weak.upgrade() else {
                return;
            };
            let should_ack = {
                let state = tab.state.borrow();
                !state.ui.banner_syncing
                    && matches!(
                        state.ui.visible_banner,
                        VisibleBannerState::External | VisibleBannerState::Missing
                    )
            };
            if should_ack {
                tab.acknowledge_pending_external();
            }
        });

        let weak = Rc::downgrade(self);
        self.scrolled
            .vadjustment()
            .connect_page_size_notify(move |_| {
                let Some(tab) = weak.upgrade() else {
                    return;
                };
                tab.refresh_scroll_past_end_padding();
            });

        let weak = Rc::downgrade(self);
        install_file_drop_target(&self.root, &weak);
        install_file_drop_target(&self.text_view, &weak);
        install_file_drop_target(&self.preview_view, &weak);
        self.install_markdown_preview_interactions();
    }
}

fn install_file_drop_target(widget: &impl IsA<gtk4::Widget>, weak: &std::rc::Weak<EditorTab>) {
    let drop_target = gtk4::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    drop_target.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let weak = weak.clone();
    drop_target.connect_drop(move |_, value, _, _| {
        let Some(tab) = weak.upgrade() else {
            return false;
        };
        let handler = tab.on_file_drop.borrow().clone();
        let Some(handler) = handler else {
            return false;
        };
        match value.get::<gdk::FileList>() {
            Ok(file_list) => {
                handler(file_list.files());
                true
            }
            Err(_) => false,
        }
    });
    widget.add_controller(drop_target);
}
