use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::glib;

pub(super) const DEBOUNCE: Duration = Duration::from_millis(250);
pub(super) const MIN_INTERVAL: Duration = Duration::from_secs(1);
pub(super) const LOCK_WAIT_LIMIT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScheduledRefresh {
    Normal,
    LockWaitExpired,
}

fn lock_wait_state(since: Option<Instant>, now: Instant) -> (Option<Instant>, bool) {
    match since {
        None => (Some(now), false),
        Some(started) => (
            Some(started),
            now.duration_since(started) >= LOCK_WAIT_LIMIT,
        ),
    }
}

#[derive(Clone)]
pub(super) struct LiveScheduler {
    pub(super) index_lock_path: PathBuf,
    pub(super) check_index_lock: bool,
    pub(super) pending: Rc<Cell<bool>>,
    pub(super) cancelled: Rc<Cell<bool>>,
    pub(super) last_refresh: Rc<RefCell<Option<Instant>>>,
    pub(super) lock_blocked_since: Rc<Cell<Option<Instant>>>,
    pub(super) lock_probe: Rc<dyn Fn(&Path) -> bool>,
    pub(super) on_refresh: Rc<dyn Fn(ScheduledRefresh)>,
}

impl LiveScheduler {
    pub(super) fn new(
        index_lock_path: PathBuf,
        check_index_lock: bool,
        on_refresh: Rc<dyn Fn(ScheduledRefresh)>,
    ) -> Self {
        Self::new_with_lock_probe(
            index_lock_path,
            check_index_lock,
            Rc::new(Path::exists),
            on_refresh,
        )
    }

    pub(super) fn new_with_lock_probe(
        index_lock_path: PathBuf,
        check_index_lock: bool,
        lock_probe: Rc<dyn Fn(&Path) -> bool>,
        on_refresh: Rc<dyn Fn(ScheduledRefresh)>,
    ) -> Self {
        Self {
            index_lock_path,
            check_index_lock,
            pending: Rc::new(Cell::new(false)),
            cancelled: Rc::new(Cell::new(false)),
            last_refresh: Rc::new(RefCell::new(None)),
            lock_blocked_since: Rc::new(Cell::new(None)),
            lock_probe,
            on_refresh,
        }
    }

    pub(super) fn schedule(&self) {
        if self.cancelled.get() {
            return;
        }
        if self.pending.replace(true) {
            return;
        }
        let delay = self.delay();
        let scheduler = self.clone();
        glib::timeout_add_local_once(delay, move || scheduler.fire());
    }

    pub(super) fn fire(&self) {
        self.pending.set(false);
        if self.cancelled.get() {
            return;
        }
        let mut kind = ScheduledRefresh::Normal;
        if self.index_lock_exists() {
            let (since, expired) = lock_wait_state(self.lock_blocked_since.get(), Instant::now());
            self.lock_blocked_since.set(since);
            if !expired {
                self.schedule();
                return;
            }
            kind = ScheduledRefresh::LockWaitExpired;
        }
        self.lock_blocked_since.set(None);
        *self.last_refresh.borrow_mut() = Some(Instant::now());
        (self.on_refresh)(kind);
    }

    pub(super) fn cancel(&self) {
        self.cancelled.set(true);
        self.pending.set(false);
    }

    pub(super) fn index_lock_exists(&self) -> bool {
        self.check_index_lock && !self.cancelled.get() && (self.lock_probe)(&self.index_lock_path)
    }

    pub(super) fn delay(&self) -> Duration {
        let Some(last) = *self.last_refresh.borrow() else {
            return DEBOUNCE;
        };
        let elapsed = last.elapsed();
        if elapsed >= MIN_INTERVAL {
            DEBOUNCE
        } else {
            MIN_INTERVAL.saturating_sub(elapsed).max(DEBOUNCE)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::time::Instant;

    use super::{DEBOUNCE, LiveScheduler};

    #[test]
    fn live_scheduler_delay_uses_debounce_and_minimum_interval() {
        let scheduler = LiveScheduler::new(
            std::path::PathBuf::from("/tmp/repo/.git/index.lock"),
            true,
            Rc::new(|_kind| {}),
        );
        assert_eq!(scheduler.delay(), DEBOUNCE);

        *scheduler.last_refresh.borrow_mut() = Some(Instant::now());
        assert!(scheduler.delay() >= DEBOUNCE);
    }

    #[test]
    fn live_scheduler_cancel_and_polling_skip_lock_probes() {
        let (scheduler, probes, refreshes) = counted_scheduler(true);
        scheduler.pending.set(true);
        scheduler.cancel();
        scheduler.fire();
        assert_eq!(probes.get(), 0);
        assert_eq!(refreshes.get(), 0);

        let (scheduler, probes, refreshes) = counted_scheduler(false);
        scheduler.fire();
        assert_eq!(probes.get(), 0);
        assert_eq!(refreshes.get(), 1);
    }

    #[test]
    fn lock_wait_gives_up_after_deadline() {
        let now = Instant::now();
        let (started, expired) = super::lock_wait_state(None, now);
        assert_eq!(started, Some(now));
        assert!(!expired);

        let (kept, expired) = super::lock_wait_state(Some(now), now);
        assert_eq!(kept, Some(now));
        assert!(!expired);

        if let Some(long_ago) = now.checked_sub(super::LOCK_WAIT_LIMIT) {
            let (_kept, expired) = super::lock_wait_state(Some(long_ago), now);
            assert!(expired);
        }
    }

    #[test]
    fn lock_wait_expiry_fires_bypass_refresh() {
        let kinds = Rc::new(RefCell::new(Vec::new()));
        let kinds_for_scheduler = Rc::clone(&kinds);
        let scheduler = LiveScheduler::new_with_lock_probe(
            PathBuf::from("/tmp/repo/.git/index.lock"),
            true,
            Rc::new(|_path| true),
            Rc::new(move |kind| kinds_for_scheduler.borrow_mut().push(kind)),
        );
        if let Some(long_ago) = Instant::now().checked_sub(super::LOCK_WAIT_LIMIT) {
            scheduler.lock_blocked_since.set(Some(long_ago));
            scheduler.fire();
            assert_eq!(
                *kinds.borrow(),
                vec![super::ScheduledRefresh::LockWaitExpired]
            );
        }
    }

    fn counted_scheduler(check_index_lock: bool) -> (LiveScheduler, Rc<Cell<i32>>, Rc<Cell<i32>>) {
        let probes = Rc::new(Cell::new(0));
        let refreshes = Rc::new(Cell::new(0));
        let probes_for_scheduler = probes.clone();
        let refreshes_for_scheduler = refreshes.clone();
        let path = if check_index_lock {
            "/tmp/repo/.git/index.lock"
        } else {
            "/run/user/1000/doc/repo/.git/index.lock"
        };
        let scheduler = LiveScheduler::new_with_lock_probe(
            PathBuf::from(path),
            check_index_lock,
            Rc::new(move |_path| {
                probes_for_scheduler.set(probes_for_scheduler.get() + 1);
                true
            }),
            Rc::new(move |_kind| {
                refreshes_for_scheduler.set(refreshes_for_scheduler.get() + 1);
            }),
        );
        (scheduler, probes, refreshes)
    }
}
