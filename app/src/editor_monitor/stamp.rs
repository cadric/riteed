use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};

use super::ExternalFileEvent;

const FILE_POLL_ATTRIBUTES: &str =
    "standard::type,standard::size,time::modified,time::modified-usec";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StampPurpose {
    Baseline,
    Poll,
    Change,
    MissingSettle,
}

impl StampPurpose {
    fn priority(self) -> u8 {
        match self {
            Self::Baseline => 0,
            Self::Poll => 1,
            Self::Change => 2,
            Self::MissingSettle => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileStamp {
    file_type: gio::FileType,
    size: i64,
    modified: u64,
    modified_usec: u32,
}

impl FileStamp {
    fn from_info(info: &gio::FileInfo) -> Self {
        Self {
            file_type: info.file_type(),
            size: info.size(),
            modified: info.attribute_uint64("time::modified"),
            modified_usec: info.attribute_uint32("time::modified-usec"),
        }
    }

    #[cfg(test)]
    pub(super) fn for_tests(modified: u64, size: i64) -> Self {
        Self {
            file_type: gio::FileType::Regular,
            size,
            modified,
            modified_usec: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum StampQueryResult {
    Present(FileStamp),
    Missing,
    Unknown,
    Cancelled,
}

impl StampQueryResult {
    fn from_async_result(result: Result<gio::FileInfo, glib::Error>) -> Self {
        match result {
            Ok(info) => Self::Present(FileStamp::from_info(&info)),
            Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => Self::Cancelled,
            Err(error) if error.matches(gio::IOErrorEnum::NotFound) => Self::Missing,
            Err(_) => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StampRequest {
    pub(super) generation: u64,
    pub(super) purpose: StampPurpose,
}

pub(super) struct StampTransition {
    pub(super) event: Option<ExternalFileEvent>,
    pub(super) next_request: Option<StampRequest>,
}

impl StampTransition {
    fn none() -> Self {
        Self {
            event: None,
            next_request: None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum ObservedStamp {
    #[default]
    Unknown,
    Present(FileStamp),
    Missing,
}

#[derive(Default)]
pub(super) struct StampMachine {
    observed: ObservedStamp,
    in_flight: Option<StampRequest>,
    pending: Option<StampPurpose>,
    generation: u64,
    cancelled: bool,
    change_after_unknown_baseline: bool,
}

impl StampMachine {
    pub(super) fn queue(&mut self, purpose: StampPurpose) -> Option<StampRequest> {
        if self.cancelled {
            return None;
        }
        if matches!(purpose, StampPurpose::Change) && self.baseline_is_unsettled() {
            self.change_after_unknown_baseline = true;
        }
        if self.in_flight.is_some() {
            self.queue_pending(purpose);
            return None;
        }
        Some(self.start_request(purpose))
    }

    pub(super) fn complete(
        &mut self,
        request: StampRequest,
        result: StampQueryResult,
    ) -> StampTransition {
        if self.cancelled || self.in_flight != Some(request) {
            return StampTransition::none();
        }
        self.in_flight = None;
        let event = self.apply_result(request.purpose, result);
        let next_request = self
            .pending
            .take()
            .map(|purpose| self.start_request(purpose));
        StampTransition {
            event,
            next_request,
        }
    }

    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
        self.in_flight = None;
        self.pending = None;
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn baseline_is_unsettled(&self) -> bool {
        matches!(self.observed, ObservedStamp::Unknown)
            || self
                .in_flight
                .is_some_and(|request| request.purpose == StampPurpose::Baseline)
    }

    fn queue_pending(&mut self, purpose: StampPurpose) {
        match self.pending {
            Some(current) if current.priority() >= purpose.priority() => {}
            _ => self.pending = Some(purpose),
        }
    }

    fn start_request(&mut self, purpose: StampPurpose) -> StampRequest {
        self.generation = self.generation.saturating_add(1);
        let request = StampRequest {
            generation: self.generation,
            purpose,
        };
        self.in_flight = Some(request);
        request
    }

    fn apply_result(
        &mut self,
        purpose: StampPurpose,
        result: StampQueryResult,
    ) -> Option<ExternalFileEvent> {
        match purpose {
            StampPurpose::Baseline => self.apply_baseline(result),
            StampPurpose::Poll => self.apply_poll(result),
            StampPurpose::Change => self.apply_change(result),
            StampPurpose::MissingSettle => self.apply_missing_settle(result),
        }
    }

    fn apply_baseline(&mut self, result: StampQueryResult) -> Option<ExternalFileEvent> {
        match result {
            StampQueryResult::Present(stamp) => self.observed = ObservedStamp::Present(stamp),
            StampQueryResult::Missing => self.observed = ObservedStamp::Missing,
            StampQueryResult::Unknown | StampQueryResult::Cancelled => {}
        }
        None
    }

    fn apply_poll(&mut self, result: StampQueryResult) -> Option<ExternalFileEvent> {
        match (&self.observed, result) {
            (_, StampQueryResult::Cancelled | StampQueryResult::Unknown)
            | (ObservedStamp::Missing, StampQueryResult::Missing) => None,
            (ObservedStamp::Unknown, StampQueryResult::Present(stamp)) => {
                self.observed = ObservedStamp::Present(stamp);
                None
            }
            (ObservedStamp::Unknown, StampQueryResult::Missing) => {
                self.observed = ObservedStamp::Missing;
                None
            }
            (ObservedStamp::Present(previous), StampQueryResult::Present(stamp))
                if previous == &stamp =>
            {
                None
            }
            (
                ObservedStamp::Present(_) | ObservedStamp::Missing,
                StampQueryResult::Present(stamp),
            ) => {
                self.observed = ObservedStamp::Present(stamp);
                Some(ExternalFileEvent::ContentPossiblyChanged)
            }
            (ObservedStamp::Present(_), StampQueryResult::Missing) => {
                self.observed = ObservedStamp::Missing;
                Some(ExternalFileEvent::Missing)
            }
        }
    }

    fn apply_change(&mut self, result: StampQueryResult) -> Option<ExternalFileEvent> {
        let force_change = self.change_after_unknown_baseline;
        self.change_after_unknown_baseline = false;
        match result {
            StampQueryResult::Present(stamp) => {
                let changed = force_change
                    || match &self.observed {
                        ObservedStamp::Unknown | ObservedStamp::Missing => true,
                        ObservedStamp::Present(previous) => previous != &stamp,
                    };
                self.observed = ObservedStamp::Present(stamp);
                changed.then_some(ExternalFileEvent::ContentPossiblyChanged)
            }
            StampQueryResult::Missing | StampQueryResult::Unknown => {
                Some(ExternalFileEvent::ContentPossiblyChanged)
            }
            StampQueryResult::Cancelled => None,
        }
    }

    fn apply_missing_settle(&mut self, result: StampQueryResult) -> Option<ExternalFileEvent> {
        match result {
            StampQueryResult::Present(stamp) => {
                self.observed = ObservedStamp::Present(stamp);
                Some(ExternalFileEvent::ContentPossiblyChanged)
            }
            StampQueryResult::Missing => {
                self.observed = ObservedStamp::Missing;
                Some(ExternalFileEvent::Missing)
            }
            StampQueryResult::Unknown => Some(ExternalFileEvent::ContentPossiblyChanged),
            StampQueryResult::Cancelled => None,
        }
    }
}

pub(super) struct StampTracker {
    file: gio::File,
    cancellable: gio::Cancellable,
    machine: RefCell<StampMachine>,
    on_event: Rc<dyn Fn(ExternalFileEvent)>,
}

impl StampTracker {
    pub(super) fn new(file: &gio::File, on_event: Rc<dyn Fn(ExternalFileEvent)>) -> Rc<Self> {
        Rc::new(Self {
            file: file.clone(),
            cancellable: gio::Cancellable::new(),
            machine: RefCell::new(StampMachine::default()),
            on_event,
        })
    }

    pub(super) fn queue(self: &Rc<Self>, purpose: StampPurpose) {
        let request = self.machine.borrow_mut().queue(purpose);
        if let Some(request) = request {
            self.start_query(request);
        }
    }

    pub(super) fn cancel(&self) {
        self.machine.borrow_mut().cancel();
        self.cancellable.cancel();
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.machine.borrow().is_cancelled()
    }

    fn start_query(self: &Rc<Self>, request: StampRequest) {
        let tracker = self.clone();
        self.file.query_info_async(
            FILE_POLL_ATTRIBUTES,
            gio::FileQueryInfoFlags::NONE,
            glib::Priority::default(),
            Some(&self.cancellable),
            move |result| {
                tracker.finish_query(request, StampQueryResult::from_async_result(result));
            },
        );
    }

    fn finish_query(self: &Rc<Self>, request: StampRequest, result: StampQueryResult) {
        let transition = self.machine.borrow_mut().complete(request, result);
        if let Some(next_request) = transition.next_request {
            self.start_query(next_request);
        }
        if let Some(event) = transition.event {
            (self.on_event)(event);
        }
    }
}
