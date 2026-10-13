//! The Runs in progress, so the UI can stop one by its id.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use toolbox_core::run::{CancellationToken, RunEnding};
use toolbox_core::target::AddressFamily;

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

/// The IP version the Technician allowed, as the UI sends it.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AddressFamilyInput {
    Auto,
    Ipv4,
    Ipv6,
}

impl From<AddressFamilyInput> for AddressFamily {
    fn from(family: AddressFamilyInput) -> Self {
        match family {
            AddressFamilyInput::Auto => AddressFamily::Auto,
            AddressFamilyInput::Ipv4 => AddressFamily::Ipv4,
            AddressFamilyInput::Ipv6 => AddressFamily::Ipv6,
        }
    }
}

/// How a Run finished, as the UI receives it.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EndingView {
    ending: &'static str,
    failure_reason: Option<String>,
}

impl From<RunEnding> for EndingView {
    fn from(ending: RunEnding) -> Self {
        let (ending, failure_reason) = match ending {
            RunEnding::Completed => ("completed", None),
            RunEnding::Stopped => ("stopped", None),
            RunEnding::ResolutionFailed { reason } => ("resolutionFailed", Some(reason)),
        };
        Self {
            ending,
            failure_reason,
        }
    }
}
