//! Bounded scratch-file work. Producers, observers and UI remain on the caller
//! thread; every writer is joined before an error/cancellation can return.
use anyhow::{anyhow, ensure, Result};
use std::sync::{mpsc, Mutex};

pub(crate) fn workers(count: usize) -> usize {
    if count < 8 {
        1
    } else {
        std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(4)
    }
}

pub(crate) fn run<T: Send, R: Send>(
    jobs: impl Iterator<Item = Result<T>>,
    worker_count: usize,
    work: impl Fn(T) -> Result<R> + Sync,
    mut observed: impl FnMut(&R, usize),
) -> Result<Vec<R>> {
    try_run(jobs, worker_count, work, |result, count| {
        observed(result, count);
        Ok(())
    })
}

pub(crate) fn try_run<T: Send, R: Send>(
    jobs: impl Iterator<Item = Result<T>>,
    worker_count: usize,
    work: impl Fn(T) -> Result<R> + Sync,
    mut observed: impl FnMut(&R, usize) -> Result<()>,
) -> Result<Vec<R>> {
    ensure!((1..=4).contains(&worker_count), "local I/O worker bound");
    if worker_count == 1 {
        let mut results = Vec::new();
        for job in jobs {
            let result = work(job?)?;
            observed(&result, results.len() + 1)?;
            results.push(result);
        }
        return Ok(results);
    }
    let (input, receiver) = mpsc::sync_channel::<(usize, T)>(worker_count * 2);
    let receiver = Mutex::new(receiver);
    let (sender, output) = mpsc::channel();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..worker_count {
            let receiver = &receiver;
            let work = &work;
            let sender = sender.clone();
            handles.push(scope.spawn(move || loop {
                let job = receiver.lock().unwrap_or_else(|p| p.into_inner()).recv();
                let Ok((index, job)) = job else { break };
                if sender.send((index, work(job))).is_err() {
                    break;
                }
            }));
        }
        drop(sender);
        let mut results = Vec::new();
        let mut scheduled = 0;
        let mut collect = |(index, result): (usize, Result<R>)| -> Result<()> {
            let result = result?;
            observed(&result, results.len() + 1)?;
            results.push((index, result));
            Ok(())
        };
        let produced = (|| -> Result<()> {
            for (index, job) in jobs.enumerate() {
                input
                    .send((index, job?))
                    .map_err(|_| anyhow!("local I/O workers stopped"))?;
                scheduled += 1;
                // Drain after every dispatch. At most the bounded input queue
                // plus active workers can finish while the producer is busy.
                for result in output.try_iter() {
                    collect(result)?;
                }
            }
            Ok(())
        })();
        drop(input);
        let completed = produced.and_then(|()| {
            for result in output.iter() {
                collect(result)?;
            }
            Ok(())
        });
        drop(collect);
        drop(output);
        // Do not leave a late writer racing the caller's cleanup or journal.
        for handle in handles {
            if handle.join().is_err() {
                return Err(anyhow!("local I/O worker panicked"));
            }
        }
        completed?;
        ensure!(
            results.len() == scheduled,
            "local I/O completion count mismatch"
        );
        results.sort_unstable_by_key(|(index, _)| *index);
        Ok(results.into_iter().map(|(_, result)| result).collect())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn out_of_order_workers_keep_manifest_order_and_observe_on_caller() {
        let caller = std::thread::current().id();
        let observed = std::cell::RefCell::new(Vec::new()); // Deliberately not Sync.
        let results = run(
            (0..64).map(Ok),
            4,
            |index| {
                std::thread::sleep(std::time::Duration::from_millis((index % 4) as u64));
                Ok(index)
            },
            |_, count| {
                assert_eq!(std::thread::current().id(), caller);
                observed.borrow_mut().push(count);
            },
        )
        .unwrap();
        assert_eq!(results, (0..64).collect::<Vec<_>>());
        assert_eq!(*observed.borrow(), (1..=64).collect::<Vec<_>>());
    }

    #[test]
    fn producer_cancel_and_worker_failure_join_all_writers_before_return() {
        for producer_failure in [true, false] {
            let active = Arc::new(AtomicUsize::new(0));
            let finished = Arc::new(AtomicUsize::new(0));
            let work_active = active.clone();
            let work_finished = finished.clone();
            let result = run(
                (0..64).map(|index| {
                    ensure!(!(producer_failure && index == 9), "cancelled on producer");
                    Ok(index)
                }),
                4,
                move |index| {
                    work_active.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(3));
                    work_active.fetch_sub(1, Ordering::SeqCst);
                    work_finished.fetch_add(1, Ordering::SeqCst);
                    ensure!(producer_failure || index != 1, "worker copy failed");
                    Ok(index)
                },
                |_, _| {},
            );
            assert!(result.is_err());
            assert_eq!(active.load(Ordering::SeqCst), 0);
            let completed = finished.load(Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(20));
            assert_eq!(finished.load(Ordering::SeqCst), completed);
            assert!(completed > 0 && completed < 64);
        }
    }

    #[test]
    fn observer_failure_joins_queued_writers_before_recovery_can_start() {
        let active = AtomicUsize::new(0);
        let finished = AtomicUsize::new(0);
        let result = try_run(
            (0..64).map(Ok),
            4,
            |index| {
                active.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(3));
                finished.fetch_add(1, Ordering::SeqCst);
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(index)
            },
            |_, count| {
                ensure!(count < 3, "interrupted completion observer");
                Ok(())
            },
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("interrupted completion"));
        assert_eq!(active.load(Ordering::SeqCst), 0);
        let completed = finished.load(Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(finished.load(Ordering::SeqCst), completed);
        assert!((3..64).contains(&completed));
    }
}
