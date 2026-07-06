use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::{SourceControlController, SourceControlState};

pub(crate) struct WeakSourceControlController {
    state: Weak<RefCell<SourceControlState>>,
}

impl SourceControlController {
    pub(crate) fn downgrade(&self) -> WeakSourceControlController {
        WeakSourceControlController {
            state: Rc::downgrade(&self.state),
        }
    }

    #[cfg(test)]
    pub(crate) fn state_weak_for_tests(&self) -> Weak<RefCell<SourceControlState>> {
        Rc::downgrade(&self.state)
    }
}

impl WeakSourceControlController {
    pub(crate) fn upgrade(&self) -> Option<SourceControlController> {
        self.state
            .upgrade()
            .map(|state| SourceControlController { state })
    }
}
