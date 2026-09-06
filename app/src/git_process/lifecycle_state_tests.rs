use super::GitProcessError;
use super::lifecycle::{GitLifecycle, capture_result};

#[test]
fn io_and_successful_wait_are_both_required_in_either_order() {
    for io_first in [false, true] {
        let mut lifecycle = GitLifecycle::default();
        if io_first {
            assert!(lifecycle.settle_io());
            assert!(!lifecycle.take_terminal());
        }
        assert!(lifecycle.begin_wait());
        assert!(!lifecycle.begin_wait());
        assert!(lifecycle.finish_wait());
        assert!(lifecycle.is_reaped());
        if io_first {
            assert!(lifecycle.take_terminal());
        } else {
            assert!(!lifecycle.take_terminal(), "I/O is still required");
            assert!(lifecycle.settle_io());
            assert!(lifecycle.take_terminal());
        }
        assert!(!lifecycle.take_terminal());
        assert!(!lifecycle.finish_wait());
        assert!(!lifecycle.begin_wait());
        assert!(!lifecycle.settle_io());
    }
}

#[test]
fn cancellation_deadline_and_io_failure_preserve_the_first_reason() {
    for first in [
        GitProcessError::Cancelled,
        GitProcessError::TimedOut,
        GitProcessError::CommandFailed(String::from("I/O")),
    ] {
        let mut lifecycle = GitLifecycle::default();
        lifecycle.record_reason(first.clone());
        lifecycle.settle_io();
        lifecycle.record_reason(GitProcessError::TimedOut);
        lifecycle.record_reason(GitProcessError::Cancelled);
        assert!(lifecycle.begin_wait());
        assert!(!lifecycle.is_reaped());
        assert!(lifecycle.finish_wait());
        assert_eq!(lifecycle.reason, Some(first));
    }
}

#[test]
fn failed_waits_retain_supervision_and_report_once() {
    let mut lifecycle = GitLifecycle::default();
    assert!(lifecycle.begin_wait());
    assert!(lifecycle.wait_failed());
    assert!(!lifecycle.finish_wait());
    assert!(!lifecycle.is_reaped());
    assert!(lifecycle.begin_wait());
    assert!(!lifecycle.wait_failed());
    assert!(lifecycle.begin_wait());
    assert!(lifecycle.finish_wait());
}

#[test]
fn output_errors_and_allowed_nonzero_statuses_remain_distinct() {
    assert!(matches!(
        capture_result(None, 10, false, Some(0)),
        Err(GitProcessError::CommandFailed(_))
    ));
    assert!(matches!(
        capture_result(Some(Err(())), 10, false, Some(0)),
        Err(GitProcessError::CommandFailed(_))
    ));
    assert!(matches!(
        capture_result(Some(Ok((vec![1; 11], Vec::new()))), 10, false, Some(0)),
        Err(GitProcessError::OutputTooLarge)
    ));
    assert!(matches!(
        capture_result(
            Some(Ok((Vec::new(), vec![1; super::STDERR_CAP + 1]))),
            10,
            false,
            Some(0)
        ),
        Err(GitProcessError::OutputTooLarge)
    ));
    assert!(matches!(
        capture_result(Some(Ok((Vec::new(), Vec::new()))), 10, true, None),
        Err(GitProcessError::CommandFailed(_))
    ));
    let result = capture_result(Some(Ok((Vec::new(), Vec::new()))), 10, true, Some(7));
    assert!(matches!(result, Ok(output) if output.status == 7));
}
