//! Background tasks whose results return to the module on the UI thread.
//!
//! Threads are used instead of an async runtime: the V1 contract is a closure
//! that does blocking work and a completion callback that receives `Ctx` on
//! the UI thread. The shell calls [`crate::Runtime::poll_tasks`] every frame
//! and the task queue wakes it through egui's repaint callback.

use crate::runtime::Ctx;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub type Wake = Arc<dyn Fn() + Send + Sync>;

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
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                next: AtomicU64::new(1),
                queue: Mutex::new(VecDeque::new()),
                wake: Mutex::new(Arc::new(|| {})),
            }),
        }
    }

    pub fn set_wake(&self, wake: Wake) {
        if let Ok(mut slot) = self.inner.wake.lock() {
            *slot = wake;
        }
    }

    /// Runs `work` on a worker thread, then `done` on the UI thread with the
    /// result. The callback keeps the module id used for `Ctx::store`/`Ctx::t`.
    pub fn spawn<T, W, D>(&self, module_id: &str, work: W, done: D) -> TaskId
    where
        T: Send + 'static,
        W: FnOnce() -> T + Send + 'static,
        D: for<'a, 'b> FnOnce(&'b mut Ctx<'a>, T) + Send + 'static,
    {
        let id = TaskId(self.inner.next.fetch_add(1, Ordering::SeqCst));
        let module_id = module_id.to_owned();
        let inner = Arc::clone(&self.inner);
        let spawned = std::thread::Builder::new()
            .name(format!("forja-{id}"))
            .spawn(move || {
                let result = work();
                let completion = Completion {
                    module_id,
                    run: Box::new(move |cx| done(cx, result)),
                };
                if let Ok(mut queue) = inner.queue.lock() {
                    queue.push_back(completion);
                }
                if let Ok(wake) = inner.wake.lock() {
                    wake();
                }
            });
        if let Err(error) = spawned {
            panic!("forja task thread could not start: {error}");
        }
        id
    }

    pub(crate) fn take_completed(&self) -> Vec<Completion> {
        match self.inner.queue.lock() {
            Ok(mut queue) => queue.drain(..).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn pending(&self) -> usize {
        self.inner
            .queue
            .lock()
            .map(|queue| queue.len())
            .unwrap_or(0)
    }
}
