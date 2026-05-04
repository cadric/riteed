use super::ExternalFileEvent;
use super::stamp::{FileStamp, StampMachine, StampPurpose, StampQueryResult, StampRequest};

fn stamp(modified: u64, size: i64) -> FileStamp {
    FileStamp::for_tests(modified, size)
}

fn content(event: Option<&ExternalFileEvent>) -> bool {
    matches!(event, Some(ExternalFileEvent::ContentPossiblyChanged))
}

fn missing(event: Option<&ExternalFileEvent>) -> bool {
    matches!(event, Some(ExternalFileEvent::Missing))
}

#[test]
fn baseline_initializes_without_event() {
    let mut machine = StampMachine::default();
    let request = machine.queue(StampPurpose::Baseline);

    assert!(request.is_some());
    if let Some(request) = request {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(10, 20)));
        assert!(transition.event.is_none());
        assert!(transition.next_request.is_none());
    }
}

#[test]
fn first_poll_initializes_without_event() {
    let mut machine = StampMachine::default();
    let request = machine.queue(StampPurpose::Poll);

    assert!(request.is_some());
    if let Some(request) = request {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(10, 20)));
        assert!(transition.event.is_none());
        assert!(transition.next_request.is_none());
    }
}

#[test]
fn poll_detects_changed_missing_and_recreated_files() {
    let mut machine = StampMachine::default();
    let first = machine.queue(StampPurpose::Poll);
    if let Some(request) = first {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(10, 20)));
        assert!(transition.event.is_none());
    }

    let changed = machine.queue(StampPurpose::Poll);
    if let Some(request) = changed {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(11, 20)));
        assert!(content(transition.event.as_ref()));
    }

    let deleted = machine.queue(StampPurpose::Poll);
    if let Some(request) = deleted {
        let transition = machine.complete(request, StampQueryResult::Missing);
        assert!(missing(transition.event.as_ref()));
    }

    let recreated = machine.queue(StampPurpose::Poll);
    if let Some(request) = recreated {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(12, 20)));
        assert!(content(transition.event.as_ref()));
    }
}

#[test]
fn change_ignores_unchanged_known_stamp() {
    let mut machine = StampMachine::default();
    let baseline = machine.queue(StampPurpose::Baseline);
    if let Some(request) = baseline {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(10, 20)));
        assert!(transition.event.is_none());
    }

    let change = machine.queue(StampPurpose::Change);
    if let Some(request) = change {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(10, 20)));
        assert!(transition.event.is_none());
    }
}

#[test]
fn change_before_baseline_completion_forces_content_event() {
    let mut machine = StampMachine::default();
    let baseline = machine.queue(StampPurpose::Baseline);
    assert!(baseline.is_some());

    let change = machine.queue(StampPurpose::Change);
    assert!(change.is_none());

    if let Some(request) = baseline {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(10, 20)));
        assert!(transition.event.is_none());
        assert!(transition.next_request.is_some());
        if let Some(next_request) = transition.next_request {
            assert_eq!(next_request.purpose, StampPurpose::Change);
            let transition =
                machine.complete(next_request, StampQueryResult::Present(stamp(10, 20)));
            assert!(content(transition.event.as_ref()));
        }
    }
}

#[test]
fn missing_settle_maps_present_missing_and_unknown() {
    let mut machine = StampMachine::default();
    let present = machine.queue(StampPurpose::MissingSettle);
    if let Some(request) = present {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(10, 20)));
        assert!(content(transition.event.as_ref()));
    }

    let missing_request = machine.queue(StampPurpose::MissingSettle);
    if let Some(request) = missing_request {
        let transition = machine.complete(request, StampQueryResult::Missing);
        assert!(missing(transition.event.as_ref()));
    }

    let unknown_request = machine.queue(StampPurpose::MissingSettle);
    if let Some(request) = unknown_request {
        let transition = machine.complete(request, StampQueryResult::Unknown);
        assert!(content(transition.event.as_ref()));
    }
}

#[test]
fn pending_priority_prefers_missing_settle() {
    let mut machine = StampMachine::default();
    let poll = machine.queue(StampPurpose::Poll);
    assert!(poll.is_some());

    assert!(machine.queue(StampPurpose::Change).is_none());
    assert!(machine.queue(StampPurpose::MissingSettle).is_none());
    assert!(machine.queue(StampPurpose::Poll).is_none());

    if let Some(request) = poll {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(10, 20)));
        assert!(transition.next_request.is_some());
        if let Some(next_request) = transition.next_request {
            assert_eq!(next_request.purpose, StampPurpose::MissingSettle);
        }
    }
}

#[test]
fn stale_requests_are_ignored() {
    let mut machine = StampMachine::default();
    let request = machine.queue(StampPurpose::Poll);
    assert!(request.is_some());

    if let Some(request) = request {
        let stale = StampRequest {
            generation: request.generation.saturating_add(1),
            purpose: request.purpose,
        };
        let transition = machine.complete(stale, StampQueryResult::Present(stamp(10, 20)));
        assert!(transition.event.is_none());
        assert!(transition.next_request.is_none());

        let transition = machine.complete(request, StampQueryResult::Present(stamp(10, 20)));
        assert!(transition.event.is_none());
    }
}

#[test]
fn cancelled_machine_ignores_late_completion() {
    let mut machine = StampMachine::default();
    let request = machine.queue(StampPurpose::Change);
    assert!(request.is_some());
    machine.cancel();
    assert!(machine.is_cancelled());

    if let Some(request) = request {
        let transition = machine.complete(request, StampQueryResult::Present(stamp(10, 20)));
        assert!(transition.event.is_none());
        assert!(transition.next_request.is_none());
    }
}
