//! The Runs in progress, so the UI can stop one by its id.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use toolbox_core::run::CancellationToken;

pub type RunId = u64;

#[derive(Default)]
pub struct Runs {
    next_id: AtomicU64,
    in_progress: Mutex<HashMap<RunId, CancellationToken>>,
}

impl Runs {
    /// Registers a new Run and hands back its id and the token that stops it.
    pub fn start(&self) -> (RunId, CancellationToken) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let stop = CancellationToken::new();
        self.in_progress.lock().unwrap().insert(id, stop.clone());
        (id, stop)
    }

    pub fn finish(&self, id: RunId) {
        self.in_progress.lock().unwrap().remove(&id);
    }

    pub fn stop(&self, id: RunId) {
        if let Some(stop) = self.in_progress.lock().unwrap().get(&id) {
            stop.cancel();
        }
    }
}

#[tauri::command]
pub fn stop_run(id: RunId, runs: tauri::State<'_, std::sync::Arc<Runs>>) {
    runs.stop(id);
}
