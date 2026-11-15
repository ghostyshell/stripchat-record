//! Background job queue for recordings and conversions.
//!
//! Each job runs on its own thread, reports `(done, total)` progress through a
//! process-wide hook, and can be cancelled with a flag the worker polls.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub kind: String,
    pub label: String,
    pub state: JobState,
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub message: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
}

struct JobSlot {
    job: Job,
    cancel: Arc<AtomicBool>,
}

pub type ProgressHook = dyn Fn(u64, u64, u64) + Send + Sync;
static PROGRESS_HOOK: Mutex<Option<Arc<ProgressHook>>> = Mutex::new(None);

pub fn set_progress_hook(hook: Option<Arc<ProgressHook>>) {
    *PROGRESS_HOOK.lock().expect("hook lock") = hook;
}

fn fire_progress(job_id: u64, done: u64, total: u64) {
    let hook = PROGRESS_HOOK.lock().expect("hook lock").clone();
    if let Some(hook) = hook {
        hook(job_id, done, total);
    }
}

#[derive(Default)]
pub struct JobManager {
    next_id: AtomicU64,
    slots: Mutex<HashMap<u64, Arc<Mutex<JobSlot>>>>,
    order: Mutex<Vec<u64>>,
}

pub struct JobContext {
    slot: Arc<Mutex<JobSlot>>,
    cancel: Arc<AtomicBool>,
}

impl Clone for JobContext {
    fn clone(&self) -> Self {
        Self {
            slot: self.slot.clone(),
            cancel: self.cancel.clone(),
        }
    }
}

impl JobContext {
    pub fn job_id(&self) -> u64 {
        self.slot.lock().expect("job slot lock").job.id
    }

    pub fn progress(&self, done: u64, total: u64) {
        {
            let mut slot = self.slot.lock().expect("job slot lock");
            slot.job.done_bytes = done;
            slot.job.total_bytes = total;
        }
        fire_progress(self.job_id(), done, total);
    }

    pub fn message(&self, msg: impl Into<String>) {
        self.slot.lock().expect("job slot lock").job.message = msg.into();
    }

    pub fn check_cancel(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(Error::Cancelled(self.job_id()))
        } else {
            Ok(())
        }
    }
}

impl JobManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn `work` on a background thread. `on_update` fires once the job
    /// reaches a final state, on the worker thread.
    pub fn spawn<F>(
        &self,
        kind: &str,
        label: &str,
        on_update: impl Fn(Job) + Send + 'static,
        work: F,
    ) -> u64
    where
        F: FnOnce(&JobContext) -> Result<()> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel = Arc::new(AtomicBool::new(false));
        let slot = Arc::new(Mutex::new(JobSlot {
            job: Job {
                id,
                kind: kind.to_string(),
                label: label.to_string(),
                state: JobState::Running,
                done_bytes: 0,
                total_bytes: 0,
                message: String::new(),
                started_at: now(),
                finished_at: None,
            },
            cancel: cancel.clone(),
        }));
        self.slots.lock().expect("slots lock").insert(id, slot.clone());
        self.order.lock().expect("order lock").push(id);

        let ctx = JobContext {
            slot: slot.clone(),
            cancel,
        };
        std::thread::spawn(move || {
            let result = work(&ctx);
            {
                let mut slot = slot.lock().expect("slot lock");
                slot.job.state = match &result {
                    Ok(()) => JobState::Done,
                    Err(Error::Cancelled(_)) => JobState::Cancelled,
                    Err(e) => {
                        slot.job.message = e.to_string();
                        JobState::Failed
                    }
                };
                slot.job.finished_at = Some(now());
            }
            let final_job = slot.lock().expect("slot lock").job.clone();
            on_update(final_job);
        });
        id
    }

    pub fn get(&self, id: u64) -> Option<Job> {
        self.slots
            .lock()
            .expect("slots lock")
            .get(&id)
            .map(|slot| slot.lock().expect("slot lock").job.clone())
    }

    pub fn list(&self) -> Vec<Job> {
        let order = self.order.lock().expect("order lock");
        let slots = self.slots.lock().expect("slots lock");
        order
            .iter()
            .rev()
            .filter_map(|id| slots.get(id))
            .map(|slot| slot.lock().expect("slot lock").job.clone())
            .collect()
    }

    pub fn cancel(&self, id: u64) -> bool {
        if let Some(slot) = self.slots.lock().expect("slots lock").get(&id) {
            slot.lock()
                .expect("slot lock")
                .cancel
                .store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn clear_finished(&self) {
        let mut order = self.order.lock().expect("order lock");
        let mut slots = self.slots.lock().expect("slots lock");
        order.retain(|id| {
            let keep = slots
                .get(id)
                .map(|slot| {
                    slot.lock().expect("slot lock").job.state == JobState::Running
                })
                .unwrap_or(false);
            if !keep {
                slots.remove(id);
            }
            keep
        });
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn wait(mgr: &JobManager, id: u64) -> Job {
        for _ in 0..200 {
            let job = mgr.get(id).expect("job present");
            if job.state != JobState::Running {
                return job;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("job did not finish");
    }

    #[test]
    fn job_reports_and_finishes() {
        let mgr = JobManager::new();
        let id = mgr.spawn("test", "work", |_| {}, |ctx| {
            ctx.progress(50, 100);
            ctx.message("halfway");
            Ok(())
        });
        let job = wait(&mgr, id);
        assert_eq!(job.state, JobState::Done);
        assert_eq!(job.done_bytes, 50);
        assert_eq!(job.message, "halfway");
    }

    #[test]
    fn cancel_is_observed() {
        let mgr = JobManager::new();
        let id = mgr.spawn("test", "block", |_| {}, |ctx| {
            for _ in 0..500 {
                ctx.check_cancel()?;
                std::thread::sleep(Duration::from_millis(2));
            }
            Ok(())
        });
        assert!(mgr.cancel(id));
        assert_eq!(wait(&mgr, id).state, JobState::Cancelled);
    }
}
