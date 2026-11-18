//! Background tasks whose results return to the module on the UI thread.
//!
//! Work runs on a fixed pool of std threads instead of one thread per task: the
//! closure does blocking work and the completion callback receives `Ctx` on the
//! UI thread. The shell calls [`crate::Runtime::poll_tasks`] every frame and the
//! pool wakes it through egui's repaint callback.
//!
//! The pool size comes from `app.performance.worker_threads` (0 = automatic,
//! between [`MIN_AUTO_WORKERS`] and [`MAX_AUTO_WORKERS`] threads). A task that
//! panics is logged and the worker keeps taking jobs, so one bad task cannot
//! stall the queue.

use crate::logging::{self, LogLevel};
use crate::runtime::Ctx;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Lowest worker count used when the setting says "automatic" (0).
pub const MIN_AUTO_WORKERS: usize = 2;
/// Highest worker count used when the setting says "automatic" (0).
pub const MAX_AUTO_WORKERS: usize = 8;
/// Highest worker count accepted from the setting.
pub const MAX_WORKERS: usize = 32;

/// Workers for `app.performance.worker_threads`: 0 means automatic.
pub fn resolve_workers(setting: f64) -> usize {
    if setting >= 1.0 {
        (setting.round() as usize).min(MAX_WORKERS)
    } else {
        automatic_workers()
    }
}

fn automatic_workers() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(MIN_AUTO_WORKERS)
        .clamp(MIN_AUTO_WORKERS, MAX_AUTO_WORKERS)
}

type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "task-{}", self.0)
    }
}

pub(crate) struct Completion {
    pub(crate) module_id: String,
    pub(crate) run: Box<dyn for<'a, 'b> FnOnce(&'b mut Ctx<'a>) + Send>,
}

struct Inner {
    next: AtomicU64,
    queue: Mutex<VecDeque<Completion>>,
    wake: Mutex<Wake>,
    inflight: Mutex<Vec<TaskId>>,
    done_signal: Condvar,
    jobs: Mutex<mpsc::Sender<Job>>,
}

/// Locks without failing on poisoning: a panicking task must not take the
/// queue down with it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

impl Inner {
    fn acquire(&self, id: TaskId) {
        lock(&self.inflight).push(id);
    }

    fn release(&self, id: TaskId) {
        let mut active = lock(&self.inflight);
        if let Some(pos) = active.iter().position(|item| *item == id) {
            active.swap_remove(pos);
        }
        if active.is_empty() {
            self.done_signal.notify_all();
        }
    }

    fn finish(&self, completion: Completion) {
        lock(&self.queue).push_back(completion);
        (lock(&self.wake))();
    }
}

struct InflightGuard {
    id: TaskId,
    inner: Arc<Inner>,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.inner.release(self.id);
    }
}

/// Cloneable handle stored in the registry-side services and in `Ctx`.
#[derive(Clone)]
pub struct Tasks {
    inner: Arc<Inner>,
}

impl Default for Tasks {
    fn default() -> Self {
        Self::new()
    }
}

impl Tasks {
    /// Pool sized from the machine (`app.performance.worker_threads` = 0).
    pub fn new() -> Self {
        Self::with_workers(automatic_workers())
    }

    /// Pool with a fixed number of worker threads.
    pub fn with_workers(workers: usize) -> Self {
        let workers = workers.clamp(1, MAX_WORKERS);
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        for index in 0..workers {
            let receiver = Arc::clone(&receiver);
            let spawned = std::thread::Builder::new()
                .name(format!("forja-worker-{index}"))
                .spawn(move || loop {
                    // The receiver lock is only held while a worker waits for a
                    // job, so the waiting workers do not block the running ones.
                    let job = {
                        let receiver = lock(&receiver);
                        match receiver.recv() {
                            Ok(job) => job,
                            // Every sender is gone: the runtime is shutting down.
                            Err(_) => break,
                        }
                    };
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                });
            if let Err(error) = spawned {
                logging::log(
                    LogLevel::Error,
                    format!("forja: worker thread {index} could not start: {error}"),
                );
                break;
            }
        }
        Self {
            inner: Arc::new(Inner {
                next: AtomicU64::new(1),
                queue: Mutex::new(VecDeque::new()),
                wake: Mutex::new(Arc::new(|| {})),
                inflight: Mutex::new(Vec::new()),
                done_signal: Condvar::new(),
                jobs: Mutex::new(sender),
            }),
        }
    }

    pub fn set_wake(&self, wake: Wake) {
        *lock(&self.inner.wake) = wake;
    }

    /// Queues `work` on the pool, then runs `done` on the UI thread with the
    /// result. The callback keeps the module id used for `Ctx::store`/`Ctx::t`.
    ///
    /// A task that panics logs the panic and never calls `done`.
    pub fn spawn<T, W, D>(&self, module_id: &str, work: W, done: D) -> TaskId
    where
        T: Send + 'static,
        W: FnOnce() -> T + Send + 'static,
        D: for<'a, 'b> FnOnce(&'b mut Ctx<'a>, T) + Send + 'static,
    {
        let id = TaskId(self.inner.next.fetch_add(1, Ordering::SeqCst));
        let module_id = module_id.to_owned();
        let inner = Arc::clone(&self.inner);
        inner.acquire(id);
        let task_inner = Arc::clone(&inner);
        let job: Job = Box::new(move || {
            let _guard = InflightGuard {
                id,
                inner: Arc::clone(&task_inner),
            };
            let Some(result) = run_guarded(id, work) else {
                return;
            };
            task_inner.finish(Completion {
                module_id,
                run: Box::new(move |cx| done(cx, result)),
            });
        });
        let queued = match lock(&self.inner.jobs).send(job) {
            Ok(()) => true,
            Err(mpsc::SendError(job)) => {
                drop(job);
                false
            }
        };
        if !queued {
            inner.release(id);
            logging::log(
                LogLevel::Error,
                format!("forja: task {id} was dropped: the task queue is closed"),
            );
        }
        id
    }

    pub(crate) fn wait_inflight(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let mut active = lock(&self.inner.inflight);
        while !active.is_empty() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let Ok((next, result)) = self.inner.done_signal.wait_timeout(active, remaining) else {
                break;
            };
            active = next;
            if result.timed_out() {
                break;
            }
        }
    }

    pub(crate) fn take_completed(&self) -> Vec<Completion> {
        lock(&self.inner.queue).drain(..).collect()
    }

    pub fn pending(&self) -> usize {
        lock(&self.inner.queue).len()
    }

    /// Number of tasks currently queued on or running in the worker pool.
    pub fn inflight(&self) -> usize {
        lock(&self.inner.inflight).len()
    }

    /// Identifiers of tasks currently queued on or running in the worker pool.
    pub fn inflight_ids(&self) -> Vec<TaskId> {
        lock(&self.inner.inflight).clone()
    }
}

/// Runs `work`, turning a panic into a log line instead of a dead worker.
fn run_guarded<T>(id: TaskId, work: impl FnOnce() -> T) -> Option<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
        Ok(result) => Some(result),
        Err(payload) => {
            logging::log(
                LogLevel::Error,
                format!("forja: task {id} panicked: {}", panic_message(&*payload)),
            );
            None
        }
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// Pumps the completion queue until it holds `wanted` entries.
    fn collect(tasks: &Tasks, wanted: usize) -> Vec<Completion> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut done = Vec::new();
        while done.len() < wanted && Instant::now() < deadline {
            done.extend(tasks.take_completed());
            if done.len() < wanted {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        done
    }

    #[test]
    fn two_workers_finish_every_queued_task() {
        let tasks = Tasks::with_workers(2);
        let ran = Arc::new(AtomicUsize::new(0));
        for index in 0..8 {
            let ran = Arc::clone(&ran);
            tasks.spawn(
                "testing",
                move || {
                    ran.fetch_add(1, Ordering::SeqCst);
                    index
                },
                |_cx, _value| {},
            );
        }
        let done = collect(&tasks, 8);
        assert_eq!(done.len(), 8);
        assert_eq!(ran.load(Ordering::SeqCst), 8);
        assert!(tasks.take_completed().is_empty());
    }

    #[test]
    fn a_panicking_task_does_not_stop_the_queue() {
        let tasks = Tasks::with_workers(2);
        let ran = Arc::new(AtomicUsize::new(0));
        tasks.spawn(
            "testing",
            || -> u8 { panic!("provoked failure") },
            |_cx, _value| {},
        );
        for _ in 0..3 {
            let ran = Arc::clone(&ran);
            tasks.spawn(
                "testing",
                move || ran.fetch_add(1, Ordering::SeqCst),
                |_cx, _value| {},
            );
        }
        let done = collect(&tasks, 3);
        assert_eq!(done.len(), 3, "the panicking task must not deliver a result");
        assert_eq!(ran.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn worker_count_comes_from_the_setting() {
        assert_eq!(resolve_workers(3.0), 3);
        assert_eq!(resolve_workers(99.0), MAX_WORKERS);
        let automatic = resolve_workers(0.0);
        assert!((MIN_AUTO_WORKERS..=MAX_AUTO_WORKERS).contains(&automatic));
        assert_eq!(resolve_workers(0.0), automatic_workers());
    }

    #[test]
    fn a_fixed_pool_bounds_how_many_tasks_run_at_once() {
        let tasks = Tasks::with_workers(2);
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        for _ in 0..6 {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            tasks.spawn(
                "testing",
                move || {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(20));
                    active.fetch_sub(1, Ordering::SeqCst);
                },
                |_cx, _value| {},
            );
        }
        assert_eq!(collect(&tasks, 6).len(), 6);
        assert_eq!(
            peak.load(Ordering::SeqCst),
            2,
            "the pool must run exactly its two workers, never more"
        );
    }

    #[test]
    fn inflight_and_pending_track_queued_running_and_completed_tasks() {
        let tasks = Tasks::with_workers(1);
        assert_eq!(tasks.inflight(), 0);
        assert_eq!(tasks.pending(), 0);

        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let id = tasks.spawn(
            "testing",
            move || {
                let _ = entered_tx.send(());
                let _ = release_rx.recv();
                42
            },
            |_cx, _value| {},
        );
        entered_rx.recv_timeout(Duration::from_secs(5)).expect("worker entered");
        assert_eq!(tasks.inflight(), 1);
        assert_eq!(tasks.inflight_ids(), vec![id]);

        release_tx.send(()).expect("release worker");
        let deadline = Instant::now() + Duration::from_secs(5);
        while tasks.inflight() > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(tasks.inflight(), 0);
        assert!(tasks.inflight_ids().is_empty());
        assert_eq!(tasks.pending(), 1);

        let done = tasks.take_completed();
        assert_eq!(done.len(), 1);
        assert_eq!(tasks.pending(), 0);
    }
}
