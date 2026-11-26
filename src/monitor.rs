//! A single, demand-driven reader. The UI never waits for Accessibility IPC.
use crate::dock::{Error, Snapshot};
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};

pub const READY_NOTIFICATION: &str = "BadgeifyDockReadCompleted";
/// A user-triggered action runs once, then its worker exits. Unlike the badge
/// monitor, it has no request queue or reconciliation timer.
pub struct Action {
    result: Receiver<Result<(), String>>,
}
impl Action {
    pub fn start(
        action: impl FnOnce() -> Result<(), String> + Send + 'static,
        ready: impl FnOnce() + Send + 'static,
    ) -> std::io::Result<Self> {
        let (finished, result) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("badgeify-action".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action))
                    .unwrap_or_else(|_| Err("The Dock action did not complete".into()));
                if finished.send(outcome).is_ok() {
                    ready();
                }
            })?;
        Ok(Self { result })
    }
    pub fn poll(&self) -> Option<Result<(), String>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("The Dock action did not complete".into())),
        }
    }
}
pub(crate) fn notify_ready() {
    // The queued closure owns no delegate/AX pointers. A notification from a
    // retired worker can only prompt a nonblocking poll of the current monitor.
    dispatch2::DispatchQueue::main().exec_async(|| {
        objc2::rc::autoreleasepool(|_| unsafe {
            objc2_foundation::NSNotificationCenter::defaultCenter().postNotificationName_object(
                &objc2_foundation::NSString::from_str(READY_NOTIFICATION),
                None,
            );
        });
    });
}

pub struct Monitor {
    targets: HashSet<String>,
    requests: SyncSender<()>,
    results: Receiver<Result<Snapshot, Error>>,
    pending: bool,
    refresh_after_pending: bool,
}
impl Monitor {
    pub fn new(targets: HashSet<String>) -> Self {
        let reader_targets = targets.clone();
        let monitor = Self::with_factory(
            move || {
                let mut reader = crate::dock::Reader::for_apps(reader_targets);
                move || objc2::rc::autoreleasepool(|_| reader.read())
            },
            notify_ready,
        );
        monitor.with_targets(targets)
    }
    pub(crate) fn with_targets(mut self, targets: HashSet<String>) -> Self {
        self.targets = targets;
        self
    }
    pub fn watches(&self, targets: &HashSet<String>) -> bool {
        &self.targets == targets
    }
    #[cfg(test)]
    fn with_reader(read: impl FnMut() -> Result<Snapshot, Error> + Send + 'static) -> Self {
        Self::with_factory(move || read, || {})
    }
    pub(crate) fn with_factory<R>(
        factory: impl FnOnce() -> R + Send + 'static,
        ready: impl Fn() + Send + 'static,
    ) -> Self
    where
        R: FnMut() -> Result<Snapshot, Error> + 'static,
    {
        let (requests, incoming) = mpsc::sync_channel(1);
        let (finished, results) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("badgeify-dock".into())
            .spawn(move || {
                let mut read = factory();
                while incoming.recv().is_ok() {
                    if finished.send(read()).is_err() {
                        break;
                    }
                    ready();
                }
            })
            .expect("create Dock reader thread");
        Self {
            targets: HashSet::new(),
            requests,
            results,
            pending: false,
            refresh_after_pending: false,
        }
    }
    /// Consume a completion without requesting another scan. This distinction
    /// prevents completion notifications from creating a continuous read loop.
    pub fn poll(&mut self) -> Option<Result<Snapshot, Error>> {
        match self.results.try_recv() {
            Ok(result) => {
                self.pending = false;
                if std::mem::take(&mut self.refresh_after_pending) {
                    self.request();
                }
                Some(result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(Error::DockUnavailable)),
        }
    }
    /// Called only by reconciliation/startup: schedule at most one next read.
    pub fn tick(&mut self) -> Option<Result<Snapshot, Error>> {
        let result = self.poll();
        self.request();
        result
    }
    /// Coalesce lifecycle events arriving during a read into one follow-up.
    /// Timer ticks intentionally do not set this flag, avoiding a busy loop when
    /// Accessibility replies take longer than the reconciliation interval.
    pub fn invalidate(&mut self) -> Option<Result<Snapshot, Error>> {
        let result = self.poll();
        if self.pending {
            self.refresh_after_pending = true;
        } else {
            self.request();
        }
        result
    }
    fn request(&mut self) {
        if !self.pending && self.requests.try_send(()).is_ok() {
            self.pending = true;
        }
    }
}
// Dropping channels releases the worker after its current bounded AX calls finish.
// Never join on AppKit's main thread; old results cannot enter a replacement monitor.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_burst_during_read_schedules_exactly_one_followup() {
        let (release, wait) = mpsc::sync_channel(1);
        let (started, seen) = mpsc::sync_channel(2);
        let (finished, notified) = mpsc::sync_channel(2);
        let mut reads = 0;
        let mut monitor = Monitor::with_factory(
            move || {
                move || {
                    reads += 1;
                    started.send(reads).unwrap();
                    if reads == 1 {
                        wait.recv().unwrap();
                    }
                    Ok(Snapshot::default())
                }
            },
            move || {
                finished.send(()).unwrap();
            },
        );
        assert!(monitor.invalidate().is_none());
        assert_eq!(
            seen.recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            1
        );
        for _ in 0..100 {
            assert!(monitor.invalidate().is_none());
        }
        release.send(()).unwrap();
        notified
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(monitor.poll().is_some());
        assert_eq!(
            seen.recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            2
        );
        notified
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(monitor.poll().is_some());
        assert!(!monitor.pending);
        assert!(!monitor.refresh_after_pending);
        for _ in 0..100 {
            assert!(monitor.poll().is_none());
        }
        assert!(seen.try_recv().is_err());
    }
    #[test]
    fn one_shot_action_returns_without_waiting_and_delivers_errors() {
        let (release, wait) = mpsc::sync_channel(1);
        let (finished, notified) = mpsc::sync_channel(1);
        let action = Action::start(
            move || {
                wait.recv().unwrap();
                Err("fixture error".into())
            },
            move || {
                finished.send(()).unwrap();
            },
        )
        .unwrap();
        for _ in 0..100 {
            assert!(action.poll().is_none());
        }
        release.send(()).unwrap();
        notified
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert_eq!(action.poll(), Some(Err("fixture error".into())));
    }
    #[test]
    fn abandoning_action_does_not_wait_or_notify_a_retired_owner() {
        let (release, wait) = mpsc::sync_channel(1);
        let (finished, notified) = mpsc::sync_channel(1);
        let action = Action::start(
            move || {
                wait.recv().unwrap();
                Ok(())
            },
            move || {
                finished.send(()).unwrap();
            },
        )
        .unwrap();
        drop(action);
        release.send(()).unwrap();
        assert!(matches!(
            notified.recv_timeout(std::time::Duration::from_secs(2)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[test]
    fn completion_is_available_before_next_tick_and_does_not_reschedule() {
        let (wake, notified) = mpsc::channel();
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let mut monitor = Monitor::with_factory(
            move || {
                move || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Snapshot::default())
                }
            },
            move || {
                wake.send(()).unwrap();
            },
        );
        assert!(monitor.tick().is_none());
        notified
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(matches!(monitor.poll(), Some(Ok(_))));
        for _ in 0..100 {
            assert!(monitor.poll().is_none());
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(!monitor.pending);
        monitor.tick();
        notified
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(monitor.poll().is_some());
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn slow_reads_never_block_ticks_or_overlap() {
        let (release, wait) = mpsc::sync_channel(1);
        let (started, seen) = mpsc::sync_channel(1);
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let mut monitor = Monitor::with_reader(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            started.send(()).unwrap();
            wait.recv().unwrap();
            Ok(Snapshot::default())
        });
        assert!(monitor.tick().is_none());
        seen.recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        for _ in 0..100 {
            assert!(monitor.tick().is_none());
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
        // Dropping the UI side must also return before the blocked reader completes.
        drop(monitor);
        release.send(()).unwrap();
    }
    #[test]
    fn completed_errors_are_delivered_without_panicking() {
        let mut monitor = Monitor::with_reader(|| Err(Error::Permission));
        monitor.tick();
        let result = monitor
            .results
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(matches!(result, Err(Error::Permission)));
    }
}
