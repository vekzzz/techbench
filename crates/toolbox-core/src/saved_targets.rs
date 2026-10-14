//! Saved Targets: Targets the Technician keeps under a label (and an optional note) so any Tool
//! can pick them. The store lives in a folder the caller names, as one JSON file; export files
//! use the same shape, so a backup is just a copy of it.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::InputError;
use crate::target::{AddressFamily, ChosenTarget};

/// The name of the store's file inside its folder.
const FILE_NAME: &str = "saved-targets.json";

/// The version of the file format this build reads and writes.
const FORMAT_VERSION: u32 = 1;

/// A Target stored under a human label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTarget {
    #[serde(default)]
    pub id: u64,
    pub label: String,
    /// A hostname, IPv4 address or IPv6 address.
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A Saved Target as the Technician filled it in: an empty note means none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SavedTargetDraft {
    pub label: String,
    pub address: String,
    pub note: String,
}

/// What an import did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportSummary {
    pub added: usize,
    /// Entries whose label and address both matched a Saved Target already stored.
    pub skipped: usize,
}

/// The store's file and the export format.
#[derive(Serialize, Deserialize)]
struct FileShape {
    version: u32,
    targets: Vec<SavedTarget>,
}

/// The Technician's Saved Targets, kept on disk after every change.
#[derive(Debug)]
pub struct SavedTargetStore {
    file: PathBuf,
    targets: Vec<SavedTarget>,
}

impl SavedTargetStore {
    /// Opens the store in `folder`, creating the folder when it is missing. A store file that
    /// cannot be read is reported and left as it is, never overwritten.
    pub fn open(folder: &Path) -> Result<Self, InputError> {
        fs::create_dir_all(folder).map_err(|error| {
            InputError::new(format!("Could not create {}: {error}", folder.display()))
        })?;
        let file = folder.join(FILE_NAME);
        let targets = if file.exists() {
            read_file(&file).map_err(|error| {
                InputError::new(format!(
                    "The Saved Targets file {} could not be read ({}). Move or delete it to start a new list.",
                    file.display(),
                    error.message
                ))
            })?
        } else {
            vec![]
        };
        Ok(Self { file, targets })
    }

    /// The Saved Targets, in the order they were created.
    pub fn list(&self) -> &[SavedTarget] {
        &self.targets
    }

    pub fn create(&mut self, draft: SavedTargetDraft) -> Result<SavedTarget, InputError> {
        let (label, address, note) = validated(&draft)?;
        let created = SavedTarget {
            id: self.next_id(),
            label,
            address,
            note,
        };
        let mut targets = self.targets.clone();
        targets.push(created.clone());
        self.save(targets)?;
        Ok(created)
    }

    pub fn update(&mut self, id: u64, draft: SavedTargetDraft) -> Result<SavedTarget, InputError> {
        let (label, address, note) = validated(&draft)?;
        let mut targets = self.targets.clone();
        let saved = targets
            .iter_mut()
            .find(|saved| saved.id == id)
            .ok_or_else(gone)?;
        *saved = SavedTarget {
            id,
            label,
            address,
            note,
        };
        let updated = saved.clone();
        self.save(targets)?;
        Ok(updated)
    }

    pub fn delete(&mut self, id: u64) -> Result<(), InputError> {
        let mut targets = self.targets.clone();
        let before = targets.len();
        targets.retain(|saved| saved.id != id);
        if targets.len() == before {
            return Err(gone());
        }
        self.save(targets)
    }

    /// Writes every Saved Target to `path` as JSON, in the format [`import_from`](Self::import_from) reads.
    pub fn export_to(&self, path: &Path) -> Result<(), InputError> {
        write_file(path, &self.targets)
    }

    /// Adds the Saved Targets in the file at `path`, skipping any whose label and address both
    /// match one already stored. A file that is not a valid export is rejected whole, and the
    /// list is left untouched.
    pub fn import_from(&mut self, path: &Path) -> Result<ImportSummary, InputError> {
        let imported = read_file(path)?;
        let mut targets = self.targets.clone();
        let mut next_id = self.next_id();
        let mut summary = ImportSummary {
            added: 0,
            skipped: 0,
        };
        for saved in imported {
            let already_stored = targets
                .iter()
                .any(|stored| stored.label == saved.label && stored.address == saved.address);
            if already_stored {
                summary.skipped += 1;
                continue;
            }
            targets.push(SavedTarget {
                id: next_id,
                ..saved
            });
            next_id += 1;
            summary.added += 1;
        }
        if summary.added > 0 {
            self.save(targets)?;
        }
        Ok(summary)
    }

    fn next_id(&self) -> u64 {
        self.targets.iter().map(|saved| saved.id).max().unwrap_or(0) + 1
    }

    /// Writes `targets` to disk and, only when that worked, makes them the list.
    fn save(&mut self, targets: Vec<SavedTarget>) -> Result<(), InputError> {
        // Write beside the file and rename over it, so a crash never leaves half a list.
        let partial = self.file.with_extension("json.tmp");
        write_file(&partial, &targets)?;
        fs::rename(&partial, &self.file).map_err(|error| {
            InputError::new(format!("Could not save {}: {error}", self.file.display()))
        })?;
        self.targets = targets;
        Ok(())
    }
}

fn gone() -> InputError {
    InputError::new("That Saved Target no longer exists")
}

/// The trimmed label, the address (a valid hostname or IP address) and the note, if any.
fn validated(draft: &SavedTargetDraft) -> Result<(String, String, Option<String>), InputError> {
    let label = draft.label.trim();
    if label.is_empty() {
        return Err(InputError::new("give the Saved Target a label"));
    }
    let address = draft.address.trim();
    ChosenTarget::parse(address, AddressFamily::Auto)?;
    let note = draft.note.trim();
    Ok((
        label.to_string(),
        address.to_string(),
        (!note.is_empty()).then(|| note.to_string()),
    ))
}

fn write_file(path: &Path, targets: &[SavedTarget]) -> Result<(), InputError> {
    let shape = FileShape {
        version: FORMAT_VERSION,
        targets: targets.to_vec(),
    };
    let json = serde_json::to_string_pretty(&shape).expect("Saved Targets are plain data");
    fs::write(path, json)
        .map_err(|error| InputError::new(format!("Could not write {}: {error}", path.display())))
}

/// Reads and checks a Saved Targets file: every entry must be valid, or none is used.
fn read_file(path: &Path) -> Result<Vec<SavedTarget>, InputError> {
    let text = fs::read_to_string(path)
        .map_err(|error| InputError::new(format!("Could not read {}: {error}", path.display())))?;
    let shape: FileShape = serde_json::from_str(&text).map_err(|error| {
        InputError::new(format!(
            "{} is not a Saved Targets file: {error}",
            path.display()
        ))
    })?;
    if shape.version > FORMAT_VERSION {
        return Err(InputError::new(format!(
            "{} was made by a newer version of IT Tools (file format {}, this one reads up to {FORMAT_VERSION})",
            path.display(),
            shape.version
        )));
    }
    shape
        .targets
        .into_iter()
        .enumerate()
        .map(|(index, saved)| {
            let draft = SavedTargetDraft {
                label: saved.label,
                address: saved.address,
                note: saved.note.unwrap_or_default(),
            };
            let (label, address, note) = validated(&draft).map_err(|error| {
                InputError::new(format!(
                    "Entry {} (\"{}\") is not valid: {}",
                    index + 1,
                    draft.label,
                    error.message
                ))
            })?;
            Ok(SavedTarget {
                id: saved.id,
                label,
                address,
                note,
            })
        })
        .collect()
}
