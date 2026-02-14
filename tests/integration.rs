use std::fs;
use std::path::Path;
use tempfile::TempDir;
use update_chat_types::{get_types, read_types_file, update_types_in_place, update_types_to_new_path};

/// Copy a single fixture file into a TempDir and return the path inside it.
fn copy_fixture(fixture: &str, tmp: &TempDir) -> std::path::PathBuf {
    let src = Path::new(fixture);
    let dst = tmp.path().join(src.file_name().unwrap());
    fs::copy(src, &dst).unwrap();
    dst
}

/// Recursively copy a directory tree into a TempDir.
fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path);
        } else {
            fs::copy(entry.path(), &dest_path).unwrap();
        }
    }
}

#[test]
fn update_types_replaces_existing_header() {
    let tmp = TempDir::new().unwrap();
    let cha = copy_fixture("fixtures/tiny-types.cha", &tmp);
    let new_types = "@Types:\tlong, toyplay, REPLACED";

    let updated = update_types_to_new_path(&cha, &cha, new_types, false).unwrap();
    assert!(updated);

    let result = get_types(&cha).unwrap();
    assert_eq!(result, Some(new_types.to_owned()));
}

#[test]
fn update_types_splices_when_missing() {
    let tmp = TempDir::new().unwrap();
    let cha = copy_fixture("fixtures/no-types.cha", &tmp);
    let new_types = "@Types:\tlong, toyplay, SPLICED";

    let updated = update_types_to_new_path(&cha, &cha, new_types, false).unwrap();
    assert!(updated);

    let result = get_types(&cha).unwrap();
    assert_eq!(result, Some(new_types.to_owned()));
}

#[test]
fn update_types_noop_when_already_matching() {
    let tmp = TempDir::new().unwrap();
    let cha = copy_fixture("fixtures/tiny-types.cha", &tmp);
    let original_bytes = fs::read(&cha).unwrap();
    let new_types = "@Types:\tlong, toyplay, TD"; // matches existing

    let updated = update_types_to_new_path(&cha, &cha, new_types, false).unwrap();
    assert!(!updated);

    // File should be byte-for-byte identical.
    let after_bytes = fs::read(&cha).unwrap();
    assert_eq!(original_bytes, after_bytes);
}

#[test]
fn update_types_dry_run_does_not_modify() {
    let tmp = TempDir::new().unwrap();
    let cha = copy_fixture("fixtures/tiny-types.cha", &tmp);
    let original_bytes = fs::read(&cha).unwrap();
    let new_types = "@Types:\tlong, toyplay, CHANGED";

    let updated = update_types_to_new_path(&cha, &cha, new_types, true).unwrap();
    assert!(updated); // Would change

    // File should be unchanged.
    let after_bytes = fs::read(&cha).unwrap();
    assert_eq!(original_bytes, after_bytes);
}

#[test]
fn update_types_in_place_full_directory() {
    let tmp = TempDir::new().unwrap();
    let test_dir = tmp.path().join("test-dir");
    copy_dir_all(Path::new("fixtures/test-dir"), &test_dir);

    let updated_files = update_types_in_place(&test_dir, false).unwrap();

    // b1.cha: oldb → b (updated)
    let b1 = get_types(&test_dir.join("b/b1.cha")).unwrap();
    assert_eq!(b1, Some("@Types:\tlong, toyplay, b".to_owned()));

    // d1.cha: oldbd → b (inherits from b, updated)
    let d1 = get_types(&test_dir.join("b/d/d1.cha")).unwrap();
    assert_eq!(d1, Some("@Types:\tlong, toyplay, b".to_owned()));

    // c1.cha: oldbc → bc (updated)
    let c1 = get_types(&test_dir.join("b/c/c1.cha")).unwrap();
    assert_eq!(c1, Some("@Types:\tlong, toyplay, bc".to_owned()));

    // a1.cha: already "a" (unchanged)
    let a1 = get_types(&test_dir.join("a/a1.cha")).unwrap();
    assert_eq!(a1, Some("@Types:\tlong, toyplay, a".to_owned()));

    // a2.cha had no @Types, should be spliced with "a"
    let a2 = get_types(&test_dir.join("a/a2.cha")).unwrap();
    assert_eq!(a2, Some("@Types:\tlong, toyplay, a".to_owned()));

    // a3.cha had no @Types (utterance first), should be spliced with "a"
    let a3 = get_types(&test_dir.join("a/a3.cha")).unwrap();
    assert_eq!(a3, Some("@Types:\tlong, toyplay, a".to_owned()));

    // a1 was already correct, so at minimum b1, d1, c1, a2, a3 = 5 updated.
    // a1 should not have been updated.
    assert!(updated_files.len() >= 5);
}

// --- Snapshot tests ---

#[test]
fn snapshot_replace_existing_types() {
    let tmp = TempDir::new().unwrap();
    let cha = copy_fixture("fixtures/tiny-types.cha", &tmp);
    let new_types = "@Types:\tlong, toyplay, SNAPSHOT";

    update_types_to_new_path(&cha, &cha, new_types, false).unwrap();

    let contents = fs::read_to_string(&cha).unwrap();
    insta::assert_snapshot!(contents);
}

#[test]
fn snapshot_splice_missing_types() {
    let tmp = TempDir::new().unwrap();
    let cha = copy_fixture("fixtures/no-types.cha", &tmp);
    let new_types = "@Types:\tlong, toyplay, SNAPSHOT";

    update_types_to_new_path(&cha, &cha, new_types, false).unwrap();

    let contents = fs::read_to_string(&cha).unwrap();
    insta::assert_snapshot!(contents);
}

// --- Edge case tests ---

#[test]
fn read_types_file_empty() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("0types.txt");
    fs::write(&path, "").unwrap();

    let result = read_types_file(&path);
    assert!(result.is_err());
    let err = format!("{:#}", result.unwrap_err());
    assert!(err.contains("empty file"), "got: {err}");
}

#[test]
fn read_types_file_malformed() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("0types.txt");
    fs::write(&path, "not a types header\n").unwrap();

    let result = read_types_file(&path);
    assert!(result.is_err());
    let err = format!("{:#}", result.unwrap_err());
    assert!(err.contains("expected @Types:"), "got: {err}");
}

#[test]
fn get_types_nonexistent_file() {
    let result = get_types(Path::new("does-not-exist.cha"));
    assert!(result.is_err());
}
