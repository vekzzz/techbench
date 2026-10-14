//! Saved Targets, exercised through the toolbox core's public interface (seam S1) over a temporary folder.

use std::fs;
use std::path::Path;

use tempfile::TempDir;
use toolbox_core::saved_targets::{ImportSummary, SavedTarget, SavedTargetDraft, SavedTargetStore};

fn draft(label: &str, address: &str, note: &str) -> SavedTargetDraft {
    SavedTargetDraft {
        label: label.to_string(),
        address: address.to_string(),
        note: note.to_string(),
    }
}

fn open(folder: &Path) -> SavedTargetStore {
    SavedTargetStore::open(folder).expect("the store should open")
}

fn summary(added: usize, skipped: usize) -> ImportSummary {
    ImportSummary { added, skipped }
}

/// (label, address, note) of each Saved Target, in list order.
fn entries(store: &SavedTargetStore) -> Vec<(String, String, Option<String>)> {
    store
        .list()
        .iter()
        .map(|saved| {
            (
                saved.label.clone(),
                saved.address.clone(),
                saved.note.clone(),
            )
        })
        .collect()
}

fn entry(label: &str, address: &str, note: Option<&str>) -> (String, String, Option<String>) {
    (
        label.to_string(),
        address.to_string(),
        note.map(str::to_string),
    )
}

#[test]
fn a_new_store_has_no_saved_targets() {
    let folder = TempDir::new().unwrap();

    assert_eq!(open(folder.path()).list(), &[] as &[SavedTarget]);
}

#[test]
fn a_created_saved_target_is_listed_with_its_label_address_and_note() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());

    let created = store
        .create(draft("Core switch", "10.0.0.1", "VLAN 20, rack B"))
        .unwrap();

    assert_eq!(
        (
            created.label.as_str(),
            created.address.as_str(),
            created.note.as_deref()
        ),
        ("Core switch", "10.0.0.1", Some("VLAN 20, rack B"))
    );
    assert_eq!(store.list(), &[created]);
}

#[test]
fn the_note_is_optional() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());

    store.create(draft("Gateway", "10.0.0.1", "   ")).unwrap();

    assert_eq!(entries(&store), [entry("Gateway", "10.0.0.1", None)]);
}

#[test]
fn saved_targets_are_listed_in_the_order_they_were_created_each_with_its_own_id() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());

    let first = store.create(draft("A", "10.0.0.1", "")).unwrap();
    let second = store.create(draft("B", "fs01.corp.local", "")).unwrap();

    assert_ne!(first.id, second.id);
    assert_eq!(
        entries(&store),
        [
            entry("A", "10.0.0.1", None),
            entry("B", "fs01.corp.local", None)
        ]
    );
}

#[test]
fn surrounding_spaces_are_trimmed() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());

    store
        .create(draft("  Core switch ", " 10.0.0.1 ", " rack B "))
        .unwrap();

    assert_eq!(
        entries(&store),
        [entry("Core switch", "10.0.0.1", Some("rack B"))]
    );
}

#[test]
fn a_saved_target_needs_a_label() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());

    let error = store.create(draft("  ", "10.0.0.1", "")).unwrap_err();

    assert_eq!(error.message, "give the Saved Target a label");
    assert_eq!(store.list(), &[] as &[SavedTarget]);
}

#[test]
fn a_saved_target_needs_a_valid_hostname_or_ip_address() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());

    for address in ["", "fs01..corp.local", "300.1.1.1", "fs 01", "-bad.local"] {
        assert!(
            store.create(draft("X", address, "")).is_err(),
            "{address:?} should be rejected"
        );
    }
    for address in ["fs01.corp.local", "10.0.0.5", "fd00::5"] {
        assert!(
            store.create(draft("X", address, "")).is_ok(),
            "{address:?} should be accepted"
        );
    }
}

#[test]
fn updating_changes_the_saved_target_in_place() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());
    store.create(draft("A", "10.0.0.1", "")).unwrap();
    let second = store.create(draft("B", "10.0.0.2", "old")).unwrap();
    store.create(draft("C", "10.0.0.3", "")).unwrap();

    let updated = store
        .update(second.id, draft("B2", "10.0.0.22", ""))
        .unwrap();

    assert_eq!(updated.id, second.id);
    assert_eq!(
        entries(&store),
        [
            entry("A", "10.0.0.1", None),
            entry("B2", "10.0.0.22", None),
            entry("C", "10.0.0.3", None)
        ]
    );
}

#[test]
fn an_invalid_update_leaves_the_saved_target_as_it_was() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());
    let saved = store.create(draft("A", "10.0.0.1", "")).unwrap();

    assert!(store.update(saved.id, draft("", "10.0.0.1", "")).is_err());

    assert_eq!(entries(&store), [entry("A", "10.0.0.1", None)]);
}

#[test]
fn updating_or_deleting_a_saved_target_that_is_gone_is_an_error() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());
    let saved = store.create(draft("A", "10.0.0.1", "")).unwrap();
    store.delete(saved.id).unwrap();

    assert!(store.update(saved.id, draft("A", "10.0.0.1", "")).is_err());
    assert!(store.delete(saved.id).is_err());
}

#[test]
fn deleting_removes_only_that_saved_target() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());
    store.create(draft("A", "10.0.0.1", "")).unwrap();
    let second = store.create(draft("B", "10.0.0.2", "")).unwrap();
    store.create(draft("C", "10.0.0.3", "")).unwrap();

    store.delete(second.id).unwrap();

    assert_eq!(
        entries(&store),
        [entry("A", "10.0.0.1", None), entry("C", "10.0.0.3", None)]
    );
}

#[test]
fn saved_targets_persist_across_store_instances() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());
    store.create(draft("A", "10.0.0.1", "first")).unwrap();
    let second = store.create(draft("B", "10.0.0.2", "")).unwrap();
    let third = store.create(draft("C", "10.0.0.3", "")).unwrap();
    store
        .update(second.id, draft("B2", "10.0.0.22", ""))
        .unwrap();
    store.delete(third.id).unwrap();
    drop(store);

    let reopened = open(folder.path());

    assert_eq!(
        entries(&reopened),
        [
            entry("A", "10.0.0.1", Some("first")),
            entry("B2", "10.0.0.22", None)
        ]
    );
}

#[test]
fn ids_stay_unique_after_a_reopen() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());
    let first = store.create(draft("A", "10.0.0.1", "")).unwrap();
    drop(store);

    let mut reopened = open(folder.path());
    let second = reopened.create(draft("B", "10.0.0.2", "")).unwrap();

    assert_ne!(first.id, second.id);
}

#[test]
fn the_store_creates_its_folder_when_it_is_missing() {
    let parent = TempDir::new().unwrap();
    let folder = parent.path().join("data").join("it-tools");
    let mut store = open(&folder);

    store.create(draft("A", "10.0.0.1", "")).unwrap();

    assert_eq!(entries(&open(&folder)), [entry("A", "10.0.0.1", None)]);
}

#[test]
fn a_store_file_that_cannot_be_read_is_reported_and_left_alone() {
    let folder = TempDir::new().unwrap();
    let file = folder.path().join("saved-targets.json");
    fs::write(&file, "{ this is not json").unwrap();

    let error = SavedTargetStore::open(folder.path()).unwrap_err();

    assert!(
        error.message.contains("saved-targets.json"),
        "{}",
        error.message
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), "{ this is not json");
}

// --- Export and import -----------------------------------------------------------------------

#[test]
fn an_export_is_a_json_file_with_a_format_version() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());
    store
        .create(draft("Core switch", "10.0.0.1", "rack B"))
        .unwrap();
    let file = folder.path().join("backup.json");

    store.export_to(&file).unwrap();

    let exported: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(exported["version"], 1);
    assert_eq!(exported["targets"][0]["label"], "Core switch");
    assert_eq!(exported["targets"][0]["address"], "10.0.0.1");
    assert_eq!(exported["targets"][0]["note"], "rack B");
}

#[test]
fn an_export_imports_into_another_store_with_everything_intact() {
    let from = TempDir::new().unwrap();
    let mut source = open(from.path());
    source
        .create(draft("Core switch", "10.0.0.1", "rack B"))
        .unwrap();
    source
        .create(draft("File server", "fs01.corp.local", ""))
        .unwrap();
    let file = from.path().join("backup.json");
    source.export_to(&file).unwrap();
    let to = TempDir::new().unwrap();
    let mut destination = open(to.path());

    let result = destination.import_from(&file).unwrap();

    assert_eq!(result, summary(2, 0));
    assert_eq!(entries(&destination), entries(&source));
    assert_eq!(entries(&open(to.path())), entries(&source));
}

#[test]
fn import_adds_to_the_existing_list() {
    let from = TempDir::new().unwrap();
    let mut source = open(from.path());
    source.create(draft("Imported", "10.0.0.9", "")).unwrap();
    let file = from.path().join("backup.json");
    source.export_to(&file).unwrap();
    let to = TempDir::new().unwrap();
    let mut destination = open(to.path());
    destination.create(draft("Mine", "10.0.0.1", "")).unwrap();

    destination.import_from(&file).unwrap();

    assert_eq!(
        entries(&destination),
        [
            entry("Mine", "10.0.0.1", None),
            entry("Imported", "10.0.0.9", None)
        ]
    );
}

#[test]
fn import_skips_an_entry_whose_label_and_address_both_match_an_existing_one() {
    let from = TempDir::new().unwrap();
    let mut source = open(from.path());
    source
        .create(draft("Core switch", "10.0.0.1", "from the backup"))
        .unwrap();
    let file = from.path().join("backup.json");
    source.export_to(&file).unwrap();
    let to = TempDir::new().unwrap();
    let mut destination = open(to.path());
    destination
        .create(draft("Core switch", "10.0.0.1", "my note"))
        .unwrap();

    let result = destination.import_from(&file).unwrap();

    assert_eq!(result, summary(0, 1));
    assert_eq!(
        entries(&destination),
        [entry("Core switch", "10.0.0.1", Some("my note"))]
    );
}

#[test]
fn import_adds_an_entry_that_matches_on_only_the_label_or_only_the_address() {
    let from = TempDir::new().unwrap();
    let mut source = open(from.path());
    source
        .create(draft("Core switch", "10.0.0.99", ""))
        .unwrap();
    source.create(draft("Other name", "10.0.0.1", "")).unwrap();
    let file = from.path().join("backup.json");
    source.export_to(&file).unwrap();
    let to = TempDir::new().unwrap();
    let mut destination = open(to.path());
    destination
        .create(draft("Core switch", "10.0.0.1", ""))
        .unwrap();

    let result = destination.import_from(&file).unwrap();

    assert_eq!(result, summary(2, 0));
}

#[test]
fn importing_the_same_file_twice_adds_nothing_the_second_time() {
    let from = TempDir::new().unwrap();
    let mut source = open(from.path());
    source.create(draft("A", "10.0.0.1", "")).unwrap();
    source.create(draft("B", "10.0.0.2", "")).unwrap();
    let file = from.path().join("backup.json");
    source.export_to(&file).unwrap();
    let to = TempDir::new().unwrap();
    let mut destination = open(to.path());

    destination.import_from(&file).unwrap();
    let again = destination.import_from(&file).unwrap();

    assert_eq!((again, destination.list().len()), (summary(0, 2), 2));
}

#[test]
fn a_malformed_import_file_is_rejected_and_leaves_the_list_untouched() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());
    store.create(draft("Mine", "10.0.0.1", "")).unwrap();
    let before = fs::read_to_string(folder.path().join("saved-targets.json")).unwrap();

    for (name, contents) in [
        ("not json", "this is not json"),
        ("wrong shape", r#"{"hello": "world"}"#),
        ("no version", r#"{"targets": []}"#),
        ("newer version", r#"{"version": 99, "targets": []}"#),
        (
            "empty label",
            r#"{"version": 1, "targets": [{"id": 1, "label": "ok", "address": "10.0.0.5"}, {"id": 2, "label": "", "address": "10.0.0.6"}]}"#,
        ),
        (
            "bad address",
            r#"{"version": 1, "targets": [{"id": 1, "label": "ok", "address": "10.0.0.5"}, {"id": 2, "label": "bad", "address": "not an address!"}]}"#,
        ),
    ] {
        let file = folder.path().join("import.json");
        fs::write(&file, contents).unwrap();

        let error = store.import_from(&file).expect_err(name);

        assert!(!error.message.is_empty(), "{name}");
        assert_eq!(entries(&store), [entry("Mine", "10.0.0.1", None)], "{name}");
        assert_eq!(
            fs::read_to_string(folder.path().join("saved-targets.json")).unwrap(),
            before,
            "{name}"
        );
    }
}

#[test]
fn the_rejection_of_a_bad_entry_names_it() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());
    let file = folder.path().join("import.json");
    fs::write(
        &file,
        r#"{"version": 1, "targets": [{"id": 2, "label": "Broken one", "address": "not an address!"}]}"#,
    )
    .unwrap();

    let error = store.import_from(&file).unwrap_err();

    assert!(error.message.contains("Broken one"), "{}", error.message);
}

#[test]
fn a_file_from_a_newer_version_says_so() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());
    let file = folder.path().join("import.json");
    fs::write(&file, r#"{"version": 99, "targets": []}"#).unwrap();

    let error = store.import_from(&file).unwrap_err();

    assert!(error.message.contains("newer"), "{}", error.message);
}

#[test]
fn importing_a_file_that_does_not_exist_says_so() {
    let folder = TempDir::new().unwrap();
    let mut store = open(folder.path());

    let error = store
        .import_from(&folder.path().join("missing.json"))
        .unwrap_err();

    assert!(error.message.contains("missing.json"), "{}", error.message);
}
