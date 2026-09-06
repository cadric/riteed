use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::gio::prelude::*;

use super::{OperationSlots, cancel_queued};

fn snapshot_for(slots: &mut OperationSlots, repo: &Path) {
    let Some(refresh) = slots.begin_refresh(repo) else {
        unreachable!("refresh must start for the fixture repository");
    };
    assert!(slots.publish_snapshot(&refresh).is_some());
    assert!(slots.finish_refresh(&refresh));
}

#[test]
fn cancelled_unfinished_mutation_still_rejects_another_writer() {
    let repo = Path::new("/repo");
    let mut slots = OperationSlots::new();
    let Some(mutation) = slots.try_begin_mutation(repo) else {
        unreachable!("first mutation must acquire the owner");
    };

    mutation.cancellable().cancel();

    assert!(slots.try_begin_mutation(repo).is_none());
    assert!(slots.is_mutation_current(&mutation));
    assert!(slots.finish_mutation(&mutation));
    assert!(slots.try_begin_mutation(repo).is_some());
}

#[test]
fn foreign_and_duplicate_finish_cannot_clear_a_newer_mutation_owner() {
    let repo = Path::new("/repo");
    let mut slots = OperationSlots::new();
    let mut foreign_slots = OperationSlots::new();
    let Some(first) = slots.try_begin_mutation(repo) else {
        unreachable!("first mutation must acquire the owner");
    };
    let Some(foreign) = foreign_slots.try_begin_mutation(repo) else {
        unreachable!("foreign mutation must acquire its own controller");
    };

    assert!(!slots.finish_mutation(&foreign));
    assert!(slots.finish_mutation(&first));
    let Some(second) = slots.try_begin_mutation(repo) else {
        unreachable!("matching finish must release the owner");
    };

    assert!(!slots.finish_mutation(&first));
    assert!(slots.is_mutation_current(&second));
    assert!(slots.finish_mutation(&second));
}

#[test]
fn replacing_diff_leaves_refresh_and_mutation_owners_untouched() {
    let repo_a = Path::new("/repo-a");
    let repo_b = Path::new("/repo-b");
    let mut slots = OperationSlots::new();
    let Some(mutation) = slots.try_begin_mutation(repo_a) else {
        unreachable!("mutation must acquire the owner");
    };
    let Some(refresh) = slots.begin_refresh(repo_b) else {
        unreachable!("refresh must start");
    };
    assert!(slots.publish_snapshot(&refresh).is_some());
    let Some(first_diff) = slots.begin_diff() else {
        unreachable!("published snapshot must permit a diff");
    };
    let Some(second_diff) = slots.begin_diff() else {
        unreachable!("replacement diff must start");
    };

    assert!(slots.is_refresh_current(&refresh));
    assert!(slots.is_mutation_current(&mutation));
    assert!(!slots.is_diff_current(&first_diff));
    assert!(slots.is_diff_current(&second_diff));
    assert!(!slots.finish_diff(&first_diff));
    assert!(slots.finish_diff(&second_diff));
    assert!(!refresh.cancellable().is_cancelled());
    assert!(!first_diff.cancellable().is_cancelled());

    cancel_queued(slots.take_cancellations());

    assert!(!refresh.cancellable().is_cancelled());
    assert!(!mutation.cancellable().is_cancelled());
    assert!(first_diff.cancellable().is_cancelled());
    assert!(!second_diff.cancellable().is_cancelled());
    assert!(slots.finish_mutation(&mutation));
}

#[test]
fn mutation_invalidates_the_status_pipeline_and_its_snapshot_diff() {
    let repo = Path::new("/repo");
    let mut slots = OperationSlots::new();
    let Some(refresh) = slots.begin_refresh(repo) else {
        unreachable!("refresh must start");
    };
    let Some(snapshot) = slots.publish_snapshot(&refresh) else {
        unreachable!("current refresh must publish");
    };
    let Some(diff) = slots.begin_diff() else {
        unreachable!("published snapshot must permit a diff");
    };

    let Some(mutation) = slots.try_begin_mutation(repo) else {
        unreachable!("mutation must acquire the owner");
    };

    assert!(!slots.is_refresh_current(&refresh));
    assert!(!slots.is_snapshot_current(&snapshot));
    assert!(!slots.is_diff_current(&diff));
    assert!(slots.is_mutation_current(&mutation));
    assert!(!refresh.cancellable().is_cancelled());
    assert!(!diff.cancellable().is_cancelled());

    cancel_queued(slots.take_cancellations());

    assert!(refresh.cancellable().is_cancelled());
    assert!(diff.cancellable().is_cancelled());
    assert!(!mutation.cancellable().is_cancelled());
}

#[test]
fn root_round_trip_invalidates_ui_but_keeps_the_same_repo_writer_owned() {
    let repo_a = Path::new("/repo-a");
    let repo_b = Path::new("/repo-b");
    let mut slots = OperationSlots::new();
    snapshot_for(&mut slots, repo_a);
    let Some(old_diff) = slots.begin_diff() else {
        unreachable!("published snapshot must permit a diff");
    };
    let Some(mutation) = slots.try_begin_mutation(repo_a) else {
        unreachable!("mutation must acquire the owner");
    };

    slots.invalidate_root();
    cancel_queued(slots.take_cancellations());
    assert!(mutation.cancellable().is_cancelled());
    assert!(slots.try_begin_mutation(repo_b).is_none());
    snapshot_for(&mut slots, repo_b);
    slots.invalidate_root();

    assert!(!slots.is_diff_current(&old_diff));
    assert!(!slots.is_mutation_current(&mutation));
    assert_eq!(mutation.repo(), repo_a);
    assert!(slots.begin_refresh(repo_a).is_none());
    assert!(slots.begin_refresh(repo_a).is_none());
    assert!(slots.try_begin_mutation(repo_a).is_none());
    assert!(slots.finish_mutation(&mutation));
    assert!(!slots.mutation_root_is_current(&mutation));
    assert!(slots.begin_refresh(repo_a).is_some());
}

#[test]
fn retiring_writer_only_coalesces_refreshes_for_its_own_repository() {
    let repo_a = Path::new("/repo-a");
    let repo_b = Path::new("/repo-b");
    let mut slots = OperationSlots::new();
    let Some(mutation) = slots.try_begin_mutation(repo_a) else {
        unreachable!("mutation must acquire the owner");
    };

    assert!(slots.begin_refresh(repo_a).is_none());
    assert!(slots.begin_refresh(repo_a).is_none());
    assert!(slots.begin_refresh(repo_b).is_some());
    assert!(slots.finish_mutation(&mutation));
}

#[test]
fn root_detection_can_run_while_the_same_repo_writer_retires() {
    let repo = Path::new("/repo");
    let mut slots = OperationSlots::new();
    let Some(mutation) = slots.try_begin_mutation(repo) else {
        unreachable!("mutation must acquire the owner");
    };
    slots.invalidate_root();

    let detection = slots.begin_detection();

    assert!(slots.is_refresh_current(&detection));
    assert!(slots.finish_refresh(&detection));
    assert!(slots.begin_refresh(repo).is_none());
    assert!(slots.finish_mutation(&mutation));
    assert!(slots.begin_refresh(repo).is_some());
}

#[test]
fn mutation_activity_tracks_ownership_instead_of_cancellation_or_epoch() {
    let repo = Path::new("/repo");
    let mut slots = OperationSlots::new();
    assert!(!slots.mutation_active());
    let Some(mutation) = slots.try_begin_mutation(repo) else {
        unreachable!("mutation must acquire the owner");
    };

    mutation.cancellable().cancel();
    slots.invalidate_root();

    assert!(slots.mutation_active());
    assert!(slots.finish_mutation(&mutation));
    assert!(!slots.mutation_active());
}

#[test]
fn cancelling_refresh_leaves_diff_snapshot_and_mutation_owners_live() {
    let repo_a = Path::new("/repo-a");
    let repo_b = Path::new("/repo-b");
    let mut slots = OperationSlots::new();
    let Some(mutation) = slots.try_begin_mutation(repo_a) else {
        unreachable!("mutation must acquire the owner");
    };
    let Some(refresh) = slots.begin_refresh(repo_b) else {
        unreachable!("other-repo refresh must start");
    };
    let Some(snapshot) = slots.publish_snapshot(&refresh) else {
        unreachable!("refresh must publish its snapshot");
    };
    let Some(diff) = slots.begin_diff() else {
        unreachable!("published snapshot must permit a diff");
    };

    slots.cancel_refresh();

    assert!(!slots.is_refresh_current(&refresh));
    assert!(slots.is_snapshot_current(&snapshot));
    assert!(slots.is_diff_current(&diff));
    assert!(slots.is_mutation_current(&mutation));
    assert!(!refresh.cancellable().is_cancelled());
    cancel_queued(slots.take_cancellations());
    assert!(refresh.cancellable().is_cancelled());
    assert!(!diff.cancellable().is_cancelled());
    assert!(!mutation.cancellable().is_cancelled());
}

#[test]
fn root_change_stales_reads_before_their_cancellation_is_flushed() {
    let repo = Path::new("/repo");
    let mut slots = OperationSlots::new();
    let Some(refresh) = slots.begin_refresh(repo) else {
        unreachable!("refresh must start");
    };
    assert!(slots.publish_snapshot(&refresh).is_some());
    let Some(diff) = slots.begin_diff() else {
        unreachable!("published snapshot must permit a diff");
    };

    slots.invalidate_root();

    assert!(!slots.is_refresh_current(&refresh));
    assert!(!slots.is_diff_current(&diff));
    assert!(!refresh.cancellable().is_cancelled());
    assert!(!diff.cancellable().is_cancelled());
    cancel_queued(slots.take_cancellations());
    assert!(refresh.cancellable().is_cancelled());
    assert!(diff.cancellable().is_cancelled());
}

#[test]
fn cancellation_signals_are_flushed_after_the_slots_borrow_is_released() {
    let slots = Rc::new(RefCell::new(OperationSlots::new()));
    let repo = PathBuf::from("/repo");
    let Some(first) = slots.borrow_mut().begin_refresh(&repo) else {
        unreachable!("refresh must start");
    };
    let callback_borrowed_slots = Rc::new(RefCell::new(false));
    let slots_for_callback = Rc::clone(&slots);
    let callback_result = Rc::clone(&callback_borrowed_slots);
    first.cancellable().connect_cancelled_local(move |_| {
        let can_borrow = slots_for_callback.try_borrow_mut().is_ok();
        *callback_result.borrow_mut() = can_borrow;
    });

    let cancellations = {
        let mut borrowed = slots.borrow_mut();
        assert!(borrowed.begin_refresh(&repo).is_some());
        assert!(!first.cancellable().is_cancelled());
        borrowed.take_cancellations()
    };
    cancel_queued(cancellations);

    assert!(first.cancellable().is_cancelled());
    assert!(*callback_borrowed_slots.borrow());
}

#[test]
fn teardown_drains_read_and_mutation_owners_for_external_cancellation() {
    let repo_a = Path::new("/repo-a");
    let repo_b = Path::new("/repo-b");
    let mut slots = OperationSlots::new();
    let Some(mutation) = slots.try_begin_mutation(repo_a) else {
        unreachable!("mutation must acquire the owner");
    };
    let Some(refresh) = slots.begin_refresh(repo_b) else {
        unreachable!("refresh must start");
    };
    assert!(slots.publish_snapshot(&refresh).is_some());
    let Some(diff) = slots.begin_diff() else {
        unreachable!("published snapshot must permit a diff");
    };
    let cancellations = slots.drain_for_teardown();

    assert!(!slots.is_refresh_current(&refresh));
    assert!(!slots.is_diff_current(&diff));
    assert!(!slots.is_mutation_current(&mutation));
    assert!(!mutation.cancellable().is_cancelled());

    cancel_queued(cancellations);

    assert!(refresh.cancellable().is_cancelled());
    assert!(diff.cancellable().is_cancelled());
    assert!(mutation.cancellable().is_cancelled());
}
