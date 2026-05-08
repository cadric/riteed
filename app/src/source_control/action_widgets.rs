use gtk4::accessible::Property;
use gtk4::prelude::*;

use crate::git_status::GitActionState;

pub(crate) fn bind_action_state(button: &gtk4::Button, action_state: &GitActionState, label: &str) {
    button.set_sensitive(action_state.enabled());
    button.update_property(&[Property::Label(label)]);
    match action_state {
        GitActionState::Enabled => {
            button.set_tooltip_text(Some(label));
            button.update_property(&[Property::Description(label)]);
        }
        GitActionState::Disabled(reason) => {
            button.set_tooltip_text(Some(reason));
            button.update_property(&[Property::Description(reason)]);
        }
    }
}
