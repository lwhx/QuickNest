//! Cancellable delayed work. Queuing is distinct from successful execution.
//!
//! `DelayedTasks` is the original fire-and-forget channel queue. `TaskQueue`
//! (B9/D1, audit 2026-11-08) is the inspectable variant: every scheduled task
//! carries caller metadata and a monotonic id, stays visible as
//! Waiting/Running/Finished in `snapshot`, and a waiting task can still be
//! cancelled individually. The cancel-versus-start race is decided under the
//! same lock: once a task's state flips to Running it cannot be recalled.
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct DelayedTasks<T> {
    stop: Arc<(Mutex<bool>, Condvar)>,
    sender: mpsc::Sender<T>,
    receiver: Mutex<mpsc::Receiver<T>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl<T: Send + 'static> Default for DelayedTasks<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + 'static> DelayedTasks<T> {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            stop: Arc::new((Mutex::new(false), Condvar::new())),
            sender,
            receiver: Mutex::new(receiver),
            threads: Mutex::new(Vec::new()),
        }
    }

    pub fn schedule(&self, delay: Duration, work: impl FnOnce() -> T + Send + 'static) -> bool {
        let stop = Arc::clone(&self.stop);
        let sender = self.sender.clone();
        let mut threads = self.threads.lock().unwrap();
        if *stop.0.lock().unwrap() {
            return false;
        }
        let mut index = 0;
        while index < threads.len() {
            if threads[index].is_finished() {
                let _ = threads.swap_remove(index).join();
            } else {
                index += 1;
            }
        }
        threads.push(std::thread::spawn(move || {
            let (lock, changed) = &*stop;
            let cancelled = lock.lock().unwrap();
            let (cancelled, _) = changed
                .wait_timeout_while(cancelled, delay, |cancelled| !*cancelled)
                .unwrap();
            if *cancelled {
                return;
            }
            drop(cancelled);
            let _ = sender.send(work());
        }));
        true
    }

    pub fn drain(&self) -> Vec<T> {
        self.receiver.lock().unwrap().try_iter().collect()
    }

    /// Waiting tasks are cancelled immediately. Already executing work is
    /// allowed to finish; its operating-system request cannot be undone here.
    pub fn cancel_pending(&self) {
        *self.stop.0.lock().unwrap() = true;
        self.stop.1.notify_all();
    }
}

/// B9/D1 (audit 2026-11-08): a monotonic per-queue task id. Ids start at 1
/// and never repeat within one queue, so cancel/snapshot answers cannot be
/// confused across successive schedules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u64);

/// Why a cancel did not take effect: `Cancelled` removed a still waiting
/// task, `Running` means the work already started and cannot be recalled,
/// `NotFound` covers unknown ids and tasks that already finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelResult {
    Cancelled,
    Running,
    NotFound,
}

/// Where a scheduled task is in its lifecycle. `Finished` means the work
/// ran and its result is queued for the next `drain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Waiting,
    Running,
    Finished,
}

/// One queued task as observed at a point in time (D1: the queue is
/// visible). `due_in` is the remaining wait for Waiting tasks and zero for
/// Running/Finished ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskView<M> {
    pub id: TaskId,
    pub meta: M,
    pub due_in: Duration,
    pub state: TaskState,
}

/// Why a schedule was refused: the queue was sealed and accepts no new work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueClosed;

struct Entry<M> {
    id: TaskId,
    meta: M,
    due: Instant,
    state: TaskState,
}

struct Inner<M> {
    sealed: bool,
    next_id: u64,
    entries: Vec<Entry<M>>,
}

struct Shared<M> {
    inner: Mutex<Inner<M>>,
    changed: Condvar,
}

/// B9/D1: an inspectable, individually cancellable delayed-task queue. `M`
/// is caller metadata carried per task (display name, item id snapshot);
/// `T` is the work result collected by `drain`. Queueing still reports
/// nothing about execution (baseline 22: 排队不等于已启动) — only `drain`
/// delivers results, and the cancel/start race is decided under one lock.
pub struct TaskQueue<M, T = ()> {
    shared: Arc<Shared<M>>,
    sender: mpsc::Sender<T>,
    receiver: Mutex<mpsc::Receiver<T>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl<M: Send + 'static, T: Send + 'static> Default for TaskQueue<M, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Send + 'static, T: Send + 'static> TaskQueue<M, T> {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            shared: Arc::new(Shared {
                inner: Mutex::new(Inner {
                    sealed: false,
                    next_id: 1,
                    entries: Vec::new(),
                }),
                changed: Condvar::new(),
            }),
            sender,
            receiver: Mutex::new(receiver),
            threads: Mutex::new(Vec::new()),
        }
    }

    /// Schedules `work` to run once after `delay`, carrying `meta` for
    /// observation. Returns a fresh monotonic id, or `QueueClosed` after
    /// `seal`. Queueing never runs the work synchronously and says nothing
    /// about whether it will succeed.
    pub fn schedule_named(
        &self,
        meta: M,
        delay: Duration,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<TaskId, QueueClosed> {
        self.join_finished_threads();
        let due = Instant::now() + delay;
        let (id, shared) = {
            let mut inner = self.shared.inner.lock().unwrap();
            if inner.sealed {
                return Err(QueueClosed);
            }
            let id = TaskId(inner.next_id);
            inner.next_id += 1;
            inner.entries.push(Entry {
                id,
                meta,
                due,
                state: TaskState::Waiting,
            });
            (id, Arc::clone(&self.shared))
        };
        let sender = self.sender.clone();
        let mut threads = self.threads.lock().unwrap();
        threads.push(std::thread::spawn(move || {
            run_task(shared, id, due, sender, work);
        }));
        Ok(id)
    }

    /// Cancels one task. A Waiting task is removed under the same lock the
    /// worker uses to start execution, so either this wins (`Cancelled`) or
    /// the work already started (`Running`, which cannot be recalled).
    /// Unknown ids and already finished tasks answer `NotFound`.
    pub fn cancel(&self, id: TaskId) -> CancelResult {
        let mut inner = self.shared.inner.lock().unwrap();
        let Some(position) = inner.entries.iter().position(|entry| entry.id == id) else {
            return CancelResult::NotFound;
        };
        match inner.entries[position].state {
            TaskState::Waiting => {
                inner.entries.remove(position);
                self.shared.changed.notify_all();
                CancelResult::Cancelled
            }
            // Already executing: the operating-system request cannot be
            // undone here (same rule as DelayedTasks::cancel_pending).
            TaskState::Running => CancelResult::Running,
            TaskState::Finished => CancelResult::NotFound,
        }
    }

    /// Every queued task at `now`, oldest id first: id, metadata, remaining
    /// wait and lifecycle state (D1: 排队/已启动/已完成可见).
    pub fn snapshot(&self, now: Instant) -> Vec<TaskView<M>>
    where
        M: Clone,
    {
        let inner = self.shared.inner.lock().unwrap();
        inner
            .entries
            .iter()
            .map(|entry| TaskView {
                id: entry.id,
                meta: entry.meta.clone(),
                due_in: entry.due.saturating_duration_since(now),
                state: entry.state,
            })
            .collect()
    }

    /// Stops accepting new tasks. Already queued tasks keep their waits;
    /// the exit path follows with `cancel_waiting` and `shutdown_settle`.
    pub fn seal(&self) {
        let mut inner = self.shared.inner.lock().unwrap();
        inner.sealed = true;
        self.shared.changed.notify_all();
    }

    /// Cancels every Waiting task at once and returns how many were removed.
    /// Running work is allowed to finish.
    pub fn cancel_waiting(&self) -> usize {
        let mut inner = self.shared.inner.lock().unwrap();
        let before = inner.entries.len();
        inner
            .entries
            .retain(|entry| entry.state != TaskState::Waiting);
        let cancelled = before - inner.entries.len();
        if cancelled > 0 {
            self.shared.changed.notify_all();
        }
        cancelled
    }

    /// How many tasks are currently executing (their results may still be
    /// delivered by a later drain).
    pub fn running_count(&self) -> usize {
        let inner = self.shared.inner.lock().unwrap();
        inner
            .entries
            .iter()
            .filter(|entry| entry.state == TaskState::Running)
            .count()
    }

    /// Collects the results of finished work (B9: 已完成的结果退出前必须
    /// 处理) and clears those finished entries. Nothing that has not run is
    /// delivered here.
    pub fn drain(&self) -> Vec<T> {
        let results: Vec<T> = self.receiver.lock().unwrap().try_iter().collect();
        let mut inner = self.shared.inner.lock().unwrap();
        inner
            .entries
            .retain(|entry| entry.state != TaskState::Finished);
        results
    }

    /// Joins worker threads that already exited, so their handles do not
    /// accumulate over a long session.
    fn join_finished_threads(&self) {
        let mut threads = self.threads.lock().unwrap();
        let mut index = 0;
        while index < threads.len() {
            if threads[index].is_finished() {
                let _ = threads.swap_remove(index).join();
            } else {
                index += 1;
            }
        }
    }

    /// Repeatedly joins finished worker threads for at most `limit`, then
    /// reports how many threads are still executing (B9: 对正在执行的工作
    /// 明确有限等待).
    fn unfinished_threads_within(&self, limit: Duration) -> usize {
        let deadline = Instant::now() + limit;
        loop {
            self.join_finished_threads();
            let unfinished = self.threads.lock().unwrap().len();
            if unfinished == 0 || Instant::now() >= deadline {
                return unfinished;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// The worker loop of one scheduled task. Waits on the shared condition
/// variable until its deadline or removal; the Waiting→Running transition
/// and a concurrent `cancel` are decided under the same lock.
fn run_task<M, T>(
    shared: Arc<Shared<M>>,
    id: TaskId,
    due: Instant,
    sender: mpsc::Sender<T>,
    work: impl FnOnce() -> T,
) {
    let mut inner = shared.inner.lock().unwrap();
    loop {
        let Some(position) = inner.entries.iter().position(|entry| entry.id == id) else {
            // Cancelled (individually, by cancel_waiting, or dropped) before
            // the deadline: the work never runs.
            return;
        };
        if inner.entries[position].state != TaskState::Waiting {
            return;
        }
        let Some(remaining) = due.checked_duration_since(Instant::now()) else {
            // Deadline reached. Flipping to Running is the point of no
            // return: after this under the same lock a cancel can only
            // answer Running.
            inner.entries[position].state = TaskState::Running;
            drop(inner);
            let result = catch_unwind(AssertUnwindSafe(work));
            let mut inner = shared.inner.lock().unwrap();
            if let Some(position) = inner.entries.iter().position(|entry| entry.id == id) {
                match result {
                    Ok(value) => {
                        inner.entries[position].state = TaskState::Finished;
                        let _ = sender.send(value);
                    }
                    // A panicked task delivers no result and leaves no
                    // entry: it must not count as running forever.
                    Err(_) => {
                        inner.entries.remove(position);
                    }
                }
                shared.changed.notify_all();
            }
            return;
        };
        let (next, _) = shared.changed.wait_timeout(inner, remaining).unwrap();
        inner = next;
    }
}

/// B9: the unified exit settlement for a task queue — stop new schedules,
/// cancel waiting tasks, give executing work a bounded window (800 ms) to
/// deliver its result, drain everything completed, and report how many
/// tasks were still executing when the window closed (their results cannot
/// be awaited at exit). Returns `(drained_events, running_remaining)`.
pub fn shutdown_settle<M, T>(queue: &TaskQueue<M, T>) -> (Vec<T>, usize)
where
    M: Send + 'static,
    T: Send + 'static,
{
    const RUNNING_SETTLE_LIMIT: Duration = Duration::from_millis(800);
    queue.seal();
    let cancelled = queue.cancel_waiting();
    let _ = cancelled;
    let _unfinished = queue.unfinished_threads_within(RUNNING_SETTLE_LIMIT);
    // Count first, drain last: anything finishing between the two is
    // delivered instead of lost at exit.
    let running_remaining = queue.running_count();
    let drained = queue.drain();
    (drained, running_remaining)
}

impl<T> Drop for DelayedTasks<T> {
    fn drop(&mut self) {
        *self.stop.0.lock().unwrap() = true;
        self.stop.1.notify_all();
        for thread in self.threads.get_mut().unwrap().drain(..) {
            if thread.is_finished() {
                let _ = thread.join();
            }
        }
    }
}

impl<M, T> Drop for TaskQueue<M, T> {
    fn drop(&mut self) {
        // Dropping cancels every waiting task: their threads wake, find no
        // entry and return without running the work. Executing work keeps
        // the shared state alive through its Arc and is allowed to finish;
        // its result has nowhere to go once the receiver is gone.
        {
            let mut inner = self.shared.inner.lock().unwrap();
            inner.sealed = true;
            inner.entries.clear();
            self.shared.changed.notify_all();
        }
        for thread in self.threads.get_mut().unwrap().drain(..) {
            if thread.is_finished() {
                let _ = thread.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    #[test]
    fn cancellation_wakes_waiters_without_running_their_work() {
        let queue = DelayedTasks::<usize>::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        assert!(queue.schedule(Duration::from_secs(60), move || {
            observed.fetch_add(1, Ordering::SeqCst);
            1
        }));
        queue.cancel_pending();
        assert!(!queue.schedule(Duration::ZERO, || 2));
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(queue.drain().is_empty());
    }

    #[test]
    fn queuing_does_not_report_a_result_and_completion_is_delivered_once() {
        let queue = DelayedTasks::new();
        assert!(queue.schedule(Duration::from_millis(30), || "actual result"));
        assert!(queue.drain().is_empty());
        let until = Instant::now() + Duration::from_secs(2);
        let result = loop {
            let result = queue.drain();
            if !result.is_empty() {
                break result;
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(result, vec!["actual result"]);
        assert!(queue.drain().is_empty());
    }

    /// Spins until `condition` holds or the timeout passes, then asserts.
    /// Only used by the queue tests, with short bounded timeouts.
    fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + timeout;
        while !condition() {
            assert!(Instant::now() < deadline, "condition not met in time");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Test gate: the work signals that it started and blocks until the
    /// test releases it, so the cancel-versus-start race is deterministic.
    struct Gate {
        started: std::sync::atomic::AtomicBool,
        release: std::sync::atomic::AtomicBool,
    }

    impl Gate {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                started: std::sync::atomic::AtomicBool::new(false),
                release: std::sync::atomic::AtomicBool::new(false),
            })
        }

        /// The work body: signal started, block until released, deliver 42.
        fn run(gate: &Arc<Gate>) -> usize {
            gate.started.store(true, Ordering::SeqCst);
            while !gate.release.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(2));
            }
            42
        }
    }

    // B9/D1: every schedule gets its own id; ids never repeat or go back.
    #[test]
    fn scheduled_tasks_get_independent_monotonic_ids() {
        let queue: TaskQueue<String, usize> = TaskQueue::new();
        let first = queue
            .schedule_named("first".to_string(), Duration::from_secs(60), || 1)
            .expect("schedule");
        let second = queue
            .schedule_named("second".to_string(), Duration::from_secs(60), || 2)
            .expect("schedule");
        let third = queue
            .schedule_named("third".to_string(), Duration::from_secs(60), || 3)
            .expect("schedule");
        assert!(
            first.0 < second.0 && second.0 < third.0,
            "ids are monotonic"
        );
        assert_ne!(first, second);
        assert_ne!(second, third);

        let snapshot = queue.snapshot(Instant::now());
        assert_eq!(
            snapshot.iter().map(|view| view.id).collect::<Vec<_>>(),
            vec![first, second, third]
        );
        assert!(
            snapshot.iter().all(|view| view.state == TaskState::Waiting),
            "nothing started within the same tick"
        );
        assert_eq!(snapshot[0].meta, "first");
        assert_eq!(snapshot[1].meta, "second");
        assert_eq!(snapshot[2].meta, "third");
        assert!(
            snapshot
                .iter()
                .all(|view| view.due_in > Duration::from_secs(59)),
            "remaining waits reflect the schedule delay"
        );
        assert_eq!(queue.cancel_waiting(), 3);
        assert!(queue.snapshot(Instant::now()).is_empty());
        assert_eq!(queue.drain(), Vec::<usize>::new());
    }

    // B9/D1: the cancel-versus-start race. A waiting task cancels cleanly;
    // once the state flipped to Running under the lock, cancel can only
    // report Running — the work cannot be recalled — and a finished task is
    // no longer cancellable at all.
    #[test]
    fn cancel_racing_a_running_task_reports_running_and_waiting_reports_cancelled() {
        let queue: TaskQueue<&'static str, usize> = TaskQueue::new();
        let waiting = queue
            .schedule_named("waiter", Duration::from_secs(60), || 1)
            .expect("schedule waiting");
        assert_eq!(queue.cancel(waiting), CancelResult::Cancelled);
        assert_eq!(queue.cancel(waiting), CancelResult::NotFound, "gone");
        assert_eq!(queue.cancel(TaskId(999_999)), CancelResult::NotFound);

        let gate = Gate::new();
        let work_gate = Arc::clone(&gate);
        let running = queue
            .schedule_named("racer", Duration::from_millis(1), move || {
                Gate::run(&work_gate)
            })
            .expect("schedule running");
        wait_until(Duration::from_secs(2), || {
            queue
                .snapshot(Instant::now())
                .iter()
                .any(|view| view.id == running && view.state == TaskState::Running)
        });
        assert_eq!(queue.running_count(), 1);
        assert_eq!(queue.cancel(running), CancelResult::Running);
        // Still tracked: the task did not vanish by being cancelled.
        assert_eq!(queue.running_count(), 1);

        gate.release.store(true, Ordering::SeqCst);
        wait_until(Duration::from_secs(2), || {
            queue
                .snapshot(Instant::now())
                .iter()
                .any(|view| view.id == running && view.state == TaskState::Finished)
        });
        assert_eq!(queue.cancel(running), CancelResult::NotFound, "finished");
        assert_eq!(queue.drain(), vec![42]);
        assert!(queue.snapshot(Instant::now()).is_empty());
        assert_eq!(queue.running_count(), 0);
    }

    // B9/D1: seal stops new schedules but does not cancel tasks already
    // queued — the exit path cancels waiting work explicitly afterwards.
    #[test]
    fn seal_rejects_new_schedules_but_keeps_already_queued_tasks() {
        let queue: TaskQueue<&'static str, &'static str> = TaskQueue::new();
        queue
            .schedule_named("still-runs", Duration::from_millis(30), || "result")
            .expect("schedule before seal");
        queue.seal();
        assert_eq!(
            queue.schedule_named("late", Duration::ZERO, || "never"),
            Err(QueueClosed)
        );
        let mut results: Vec<&'static str> = Vec::new();
        wait_until(Duration::from_secs(2), || {
            results = queue.drain();
            !results.is_empty()
        });
        assert_eq!(results, vec!["result"]);
        assert!(queue.snapshot(Instant::now()).is_empty());
        assert_eq!(queue.running_count(), 0);
    }

    // B9: the exit settlement — seal, cancel waiting, bounded join on the
    // running work, drain everything finished, zero running left. Cancelled
    // waiting work never executes; the sealed queue refuses new schedules.
    #[test]
    fn shutdown_settle_cancels_waiting_drains_and_reaches_zero_running() {
        let queue: TaskQueue<&'static str, usize> = TaskQueue::new();
        let gate = Gate::new();
        let work_gate = Arc::clone(&gate);
        let running = queue
            .schedule_named("runner", Duration::from_millis(1), move || {
                Gate::run(&work_gate)
            })
            .expect("schedule running");
        let waiting_calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&waiting_calls);
        queue
            .schedule_named("waiter", Duration::from_secs(60), move || {
                observed.fetch_add(1, Ordering::SeqCst);
                8
            })
            .expect("schedule waiting");
        wait_until(Duration::from_secs(2), || {
            queue
                .snapshot(Instant::now())
                .iter()
                .any(|view| view.id == running && view.state == TaskState::Running)
        });
        gate.release.store(true, Ordering::SeqCst);

        let (drained, running_remaining) = shutdown_settle(&queue);
        assert_eq!(drained, vec![42], "the finished result is delivered");
        assert_eq!(running_remaining, 0, "no work was left executing");
        assert_eq!(queue.running_count(), 0);
        assert!(
            queue.schedule_named("late", Duration::ZERO, || 9).is_err(),
            "the queue was sealed by the settlement"
        );
        assert!(queue.snapshot(Instant::now()).is_empty());
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(
            waiting_calls.load(Ordering::SeqCst),
            0,
            "cancelled waiting work never ran"
        );

        // Settling an idle queue is a no-op.
        let idle: TaskQueue<&'static str, usize> = TaskQueue::new();
        let (drained, remaining) = shutdown_settle(&idle);
        assert!(drained.is_empty());
        assert_eq!(remaining, 0);
    }
}
