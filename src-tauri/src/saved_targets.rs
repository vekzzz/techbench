//! Saved Target commands: the store lives in the app's data folder, and the UI reads and edits it here.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use toolbox_core::saved_targets::{SavedTarget, SavedTargetDraft, SavedTargetStore};

/// The Saved Target store, or why it could not be opened (a damaged file is reported, never overwritten).
pub struct SavedTargets(Mutex<Result<SavedTargetStore, String>>);

impl SavedTargets {
    pub fn open(folder: PathBuf) -> Self {
        Self(Mutex::new(
            SavedTargetStore::open(&folder).map_err(|error| error.message),
        ))
    }

    fn with_store<T>(
        &self,
        work: impl FnOnce(&mut SavedTargetStore) -> Result<T, toolbox_core::InputError>,
    ) -> Result<T, String> {
        let mut store = self.0.lock().unwrap();
        match store.as_mut() {
            Ok(store) => work(store).map_err(|error| error.message),
            Err(reason) => Err(reason.clone()),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedTargetInput {
    label: String,
    address: String,
    note: String,
}

impl From<SavedTargetInput> for SavedTargetDraft {
    fn from(input: SavedTargetInput) -> Self {
        Self {
            label: input.label,
            address: input.address,
            note: input.note,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SavedTargetView {
    id: u64,
    label: String,
    address: String,
    note: Option<String>,
}

impl From<SavedTarget> for SavedTargetView {
    fn from(saved: SavedTarget) -> Self {
        Self {
            id: saved.id,
            label: saved.label,
            address: saved.address,
            note: saved.note,
        }
    }
}

#[derive(Serialize)]
pub struct ImportView {
    added: usize,
    skipped: usize,
}

#[tauri::command]
pub fn list_saved_targets(
    targets: tauri::State<'_, SavedTargets>,
) -> Result<Vec<SavedTargetView>, String> {
    targets.with_store(|store| Ok(store.list().iter().cloned().map(Into::into).collect()))
}

#[tauri::command]
pub fn create_saved_target(
    input: SavedTargetInput,
    targets: tauri::State<'_, SavedTargets>,
) -> Result<SavedTargetView, String> {
    targets.with_store(|store| store.create(input.into()).map(Into::into))
}

#[tauri::command]
pub fn update_saved_target(
    id: u64,
    input: SavedTargetInput,
    targets: tauri::State<'_, SavedTargets>,
) -> Result<SavedTargetView, String> {
    targets.with_store(|store| store.update(id, input.into()).map(Into::into))
}

#[tauri::command]
pub fn delete_saved_target(id: u64, targets: tauri::State<'_, SavedTargets>) -> Result<(), String> {
    targets.with_store(|store| store.delete(id))
}

/// Writes every Saved Target to the JSON file at `path`, which the UI took from a save dialog.
#[tauri::command]
pub fn export_saved_targets(
    path: String,
    targets: tauri::State<'_, SavedTargets>,
) -> Result<(), String> {
    targets.with_store(|store| store.export_to(Path::new(&path)))
}

/// Adds the Saved Targets in the JSON file at `path`, which the UI took from an open dialog.
#[tauri::command]
pub fn import_saved_targets(
    path: String,
    targets: tauri::State<'_, SavedTargets>,
) -> Result<ImportView, String> {
    targets.with_store(|store| {
        let summary = store.import_from(Path::new(&path))?;
        Ok(ImportView {
            added: summary.added,
            skipped: summary.skipped,
        })
    })
}
