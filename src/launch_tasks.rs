//! Cancellable delayed work. Queuing is distinct from successful execution.
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

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
}
