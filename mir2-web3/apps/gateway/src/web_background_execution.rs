//! Bound synchronous background work across player sockets. Tokio's
//! `block_in_place` can replace a blocked worker, so its worker count alone is
//! not a limit on concurrent session ticks and spectator snapshots.

use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, RwLock, RwLockWriteGuard, Semaphore};

#[derive(Clone)]
pub(super) struct BackgroundExecutionBudget {
    permits: Arc<Semaphore>,
    slots: usize,
}

impl BackgroundExecutionBudget {
    pub(super) fn for_current_process() -> Self {
        Self::new(process_background_slots())
    }

    pub(super) fn slots(&self) -> usize {
        self.slots
    }

    fn new(slots: usize) -> Self {
        assert!(slots > 0);
        Self {
            permits: Arc::new(Semaphore::new(slots)),
            slots,
        }
    }

    async fn acquire(&self) -> OwnedSemaphorePermit {
        Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .expect("the background execution budget is never closed")
    }

    pub(super) async fn run_spectator<T>(&self, execute: impl FnOnce() -> T) -> T {
        let _permit = self.acquire().await;
        execute()
    }

    pub(super) async fn run_tick<'gate, T>(
        &self,
        serial_gate: &'gate RwLock<()>,
        execute: impl FnOnce() -> T,
    ) -> (T, RwLockWriteGuard<'gate, ()>) {
        loop {
            let permit = self.acquire().await;
            if let Ok(serial_execution) = serial_gate.try_write() {
                let result = execute();
                // The caller retains the socket's write gate for bootstrap,
                // registration and flush, but never the shared compute permit.
                drop(permit);
                return (result, serial_execution);
            }
            // A socket's outbound reader may be waiting on a slow network send.
            // Do not let that socket reserve compute needed by other players.
            drop(permit);
            drop(serial_gate.write().await);
            // Rejoin the compute queue before trying the socket again. This
            // does not preserve the original write queue position or promise
            // starvation freedom under an unbounded stream of readers.
        }
    }
}

fn process_background_slots() -> usize {
    let estimate = std::thread::available_parallelism().map_or(1, usize::from);
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessAffinityMask};
        let mut process_mask = 0_usize;
        let mut system_mask = 0_usize;
        // Rust documents that available_parallelism may overcount Windows
        // process/job limits. Read the current process mask explicitly; the
        // isolated load-test job applies its four-CPU affinity before startup.
        let queried = unsafe {
            GetProcessAffinityMask(GetCurrentProcess(), &mut process_mask, &mut system_mask)
        };
        if queried == 0 || process_mask == 0 {
            return 1;
        }
        return estimate.min(process_mask.count_ones() as usize).max(1);
    }
    #[cfg(not(windows))]
    estimate
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use futures_util::{poll, FutureExt};
    use tokio::sync::RwLock;

    use super::BackgroundExecutionBudget;

    #[test]
    fn process_background_slots_respect_the_windows_process_mask() {
        let slots = super::process_background_slots();
        assert!(slots >= 1);
        println!("effective_background_execution_slots={slots}");
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Threading::{
                GetCurrentProcess, GetProcessAffinityMask,
            };
            let mut process_mask = 0_usize;
            let mut system_mask = 0_usize;
            assert_ne!(
                unsafe {
                    GetProcessAffinityMask(GetCurrentProcess(), &mut process_mask, &mut system_mask)
                },
                0
            );
            assert_ne!(process_mask, 0);
            assert!(slots <= process_mask.count_ones() as usize);
            println!("background_execution_process_affinity_mask={process_mask:x}");
        }
    }

    #[tokio::test]
    async fn cloned_socket_budgets_share_the_same_execution_limit() {
        let budget = BackgroundExecutionBudget::new(2);
        let another_socket = budget.clone();
        let first = budget.acquire().await;
        let second = another_socket.acquire().await;
        let executed = AtomicUsize::new(0);
        let mut snapshot = Box::pin(another_socket.run_spectator(|| {
            executed.fetch_add(1, Ordering::SeqCst);
        }));
        assert!(poll!(snapshot.as_mut()).is_pending());
        assert_eq!(executed.load(Ordering::SeqCst), 0);
        drop(first);
        tokio::time::timeout(Duration::from_secs(1), snapshot)
            .await
            .expect("freeing either socket's compute slot must wake the other socket");
        assert_eq!(executed.load(Ordering::SeqCst), 1);
        drop(second);
        assert_eq!(budget.permits.available_permits(), 2);
    }

    #[tokio::test]
    async fn queued_tick_does_not_lock_its_socket_while_compute_is_full() {
        let budget = BackgroundExecutionBudget::new(1);
        let gate = RwLock::new(());
        let occupied = budget.acquire().await;
        let executed = AtomicUsize::new(0);
        let mut tick = Box::pin(budget.run_tick(&gate, || {
            executed.fetch_add(1, Ordering::SeqCst);
        }));
        assert!(poll!(tick.as_mut()).is_pending());
        assert!(
            gate.try_read().is_ok(),
            "compute queue must leave ACK/read paths free"
        );
        drop(tick);
        drop(occupied);
        assert_eq!(executed.load(Ordering::SeqCst), 0);
        assert!(budget.permits.try_acquire().is_ok());
        assert!(gate.try_write().is_ok());
    }

    #[tokio::test]
    async fn a_blocked_socket_leaves_compute_available_for_another_socket() {
        let budget = BackgroundExecutionBudget::new(1);
        let blocked_gate = RwLock::new(());
        let reader = blocked_gate.read().await;
        let mut blocked_tick = Box::pin(budget.run_tick(&blocked_gate, || panic!("still blocked")));
        assert!(poll!(blocked_tick.as_mut()).is_pending());
        let other_gate = RwLock::new(());
        let (value, other_write) =
            tokio::time::timeout(Duration::from_secs(1), budget.run_tick(&other_gate, || 42))
                .await
                .expect("an unrelated socket must make progress");
        assert_eq!(value, 42);
        drop(other_write);
        drop(blocked_tick);
        assert!(
            blocked_gate.try_read().is_ok(),
            "cancellation removes the queued writer"
        );
        drop(reader);
        assert!(blocked_gate.try_write().is_ok());
    }

    #[tokio::test]
    async fn completed_tick_keeps_bootstrap_exclusive_but_releases_shared_compute() {
        let budget = BackgroundExecutionBudget::new(1);
        let gate = RwLock::new(());
        let executed = AtomicUsize::new(0);
        let (result, bootstrap_write) = budget
            .run_tick(&gate, || {
                executed.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>("original session failure")
            })
            .await;
        assert_eq!(result, Err("original session failure"));
        assert_eq!(executed.load(Ordering::SeqCst), 1);
        assert!(
            gate.try_read().is_err(),
            "landing packets must precede live ACKs"
        );
        let other_work = tokio::time::timeout(
            Duration::from_secs(1),
            budget.run_spectator(|| "other snapshot"),
        )
        .await
        .expect("pending bootstrap/network flush must not retain shared compute");
        assert_eq!(other_work, "other snapshot");
        drop(bootstrap_write);
        assert!(gate.try_read().is_ok());
    }

    #[tokio::test]
    async fn a_panicking_background_body_releases_both_resources() {
        let budget = BackgroundExecutionBudget::new(1);
        let gate = RwLock::new(());
        let result = std::panic::AssertUnwindSafe(budget.run_tick(&gate, || {
            panic!("synthetic background panic");
        }))
        .catch_unwind()
        .await;
        assert!(result.is_err());
        assert!(gate.try_write().is_ok());
        assert!(budget.permits.try_acquire().is_ok());
        let result = std::panic::AssertUnwindSafe(budget.run_spectator(|| {
            panic!("synthetic spectator panic");
        }))
        .catch_unwind()
        .await;
        assert!(result.is_err());
        assert!(budget.permits.try_acquire().is_ok());
    }

    #[tokio::test]
    async fn finite_reader_churn_does_not_duplicate_or_lose_a_selected_tick() {
        let budget = BackgroundExecutionBudget::new(1);
        let gate = Arc::new(RwLock::new(()));
        let first_reader = gate.read().await;
        let executed = AtomicUsize::new(0);
        let mut tick = Box::pin(budget.run_tick(&gate, || {
            executed.fetch_add(1, Ordering::SeqCst);
        }));
        assert!(poll!(tick.as_mut()).is_pending());
        let readers = (0..8)
            .map(|_| {
                let gate = Arc::clone(&gate);
                tokio::spawn(async move {
                    for _ in 0..32 {
                        let guard = gate.read().await;
                        tokio::task::yield_now().await;
                        drop(guard);
                    }
                })
            })
            .collect::<Vec<_>>();
        drop(first_reader);
        let (_, writer) = tokio::time::timeout(Duration::from_secs(2), tick)
            .await
            .expect("selected tick must finish after bounded reader traffic");
        drop(writer);
        for reader in readers {
            reader.await.unwrap();
        }
        assert_eq!(executed.load(Ordering::SeqCst), 1);
        assert!(budget.permits.try_acquire().is_ok());
    }
}
