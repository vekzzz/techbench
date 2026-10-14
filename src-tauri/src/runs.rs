//! The Runs in progress, so the UI can stop one, or copy one still going, by its id.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use toolbox_core::run::{CancellationToken, Progress, RunEnding};
use toolbox_core::target::AddressFamily;

pub type RunId = u64;

/// Produces a Run's Copy as text from its Result so far; None before it has one.
type TextSoFar = Box<dyn Fn() -> Option<String> + Send + Sync>;

struct InProgress {
    stop: CancellationToken,
    text: Option<TextSoFar>,
}

#[derive(Default)]
pub struct Runs {
    next_id: AtomicU64,
    in_progress: Mutex<HashMap<RunId, InProgress>>,
}

impl Runs {
    /// Starts `work` in the background as a new Run, handing it the token that stops it, and
    /// returns the Run's id. The Run is forgotten once `work` finishes.
    pub fn spawn<F>(self: &Arc<Self>, work: impl FnOnce(CancellationToken) -> F) -> RunId
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawn_with(None, work)
    }

    /// Like [`Runs::spawn`], for a Run that keeps `progress` up to date: while it is going, its
    /// Copy as text can be read through [`run_text`].
    pub fn spawn_copyable<R, F>(
        self: &Arc<Self>,
        progress: &Progress<R>,
        to_text: fn(&R) -> String,
        work: impl FnOnce(CancellationToken) -> F,
    ) -> RunId
    where
        R: Send + 'static,
        F: Future<Output = ()> + Send + 'static,
    {
        let progress = progress.clone();
        self.spawn_with(Some(Box::new(move || progress.read(to_text))), work)
    }

    fn spawn_with<F>(
        self: &Arc<Self>,
        text: Option<TextSoFar>,
        work: impl FnOnce(CancellationToken) -> F,
    ) -> RunId
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let stop = CancellationToken::new();
        self.in_progress.lock().unwrap().insert(
            id,
            InProgress {
                stop: stop.clone(),
                text,
            },
        );
        let work = work(stop);
        let runs = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            work.await;
            runs.in_progress.lock().unwrap().remove(&id);
        });
        id
    }

    pub fn stop(&self, id: RunId) {
        if let Some(run) = self.in_progress.lock().unwrap().get(&id) {
            run.stop.cancel();
        }
    }

    fn text(&self, id: RunId) -> Option<String> {
        let runs = self.in_progress.lock().unwrap();
        runs.get(&id)?.text.as_ref()?()
    }
}

#[tauri::command]
pub fn stop_run(id: RunId, runs: tauri::State<'_, Arc<Runs>>) {
    runs.stop(id);
}

/// The Copy as text of a Run still going; None once it has finished (its Result carries the
/// text then) or before it has anything to show.
#[tauri::command]
pub fn run_text(id: RunId, runs: tauri::State<'_, Arc<Runs>>) -> Option<String> {
    runs.text(id)
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
            RunEnding::Running => ("running", None),
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
