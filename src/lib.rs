use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use walkdir::WalkDir;

/// Extract @Types header if any, by checking one line at a time and
/// bailing out immediately after seeing 30 lines of transcript, or
/// seeing first utterance, whichever happens first. It is guaranteed
/// that no @Types header will appear after that point.
pub fn get_types(path: &Path) -> Result<Option<String>> {
    let file = File::open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    let buf_read = BufReader::new(file);
    for (i, line) in buf_read.lines().enumerate() {
        if i > 30 {
            break;
        }
        let good_line = line.with_context(|| format!("reading line from {}", path.display()))?;
        match good_line.as_bytes() {
            [b'@', b'T', b'y', b'p', b'e', b's', b':', ..] => return Ok(Some(good_line)),
            [b'*', ..] => {
                break;
            }
            _ => {}
        }
    }
    Ok(None)
}

/// What to do with a header line when updating @Types.
#[derive(Debug, PartialEq)]
pub enum HeaderAction {
    /// Replace the existing @Types line with the new value.
    Replace,
    /// The existing @Types line already matches — no change needed.
    AlreadyOk,
    /// No @Types found yet; splice the new value before this line.
    Splice,
    /// Not a relevant line; keep scanning.
    Continue,
}

/// Classify a single header line to decide what action to take.
pub fn classify_header_line(line: &str, new_types: &str) -> HeaderAction {
    match line.as_bytes() {
        [b'@', b'T', b'y', b'p', b'e', b's', b':', ..] => {
            if line == new_types {
                HeaderAction::AlreadyOk
            } else {
                HeaderAction::Replace
            }
        }
        [b'@', b'E', b'n', b'd', ..] | [b'*', ..] => HeaderAction::Splice,
        _ => HeaderAction::Continue,
    }
}

/// Update @Types header in a file, writing the result to new_path.
/// If dry_run, only check whether an update would happen.
/// Returns whether there was a change.
pub fn update_types_to_new_path(
    path: &Path,
    new_path: &Path,
    new_types: &str,
    dry_run: bool,
) -> Result<bool> {
    let file = File::open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut line_buf = String::new();

    // Accumulate prefix lines only when not dry_run.
    let mut prefix: Vec<u8> = Vec::new();

    loop {
        line_buf.clear();
        let bytes_read = reader.read_line(&mut line_buf)
            .with_context(|| format!("reading {}", path.display()))?;
        if bytes_read == 0 {
            // EOF without finding @Types or utterance — no change.
            return Ok(false);
        }

        // Trim the trailing newline for classification, but keep
        // the original line_buf (with newline) for writing.
        let trimmed = line_buf.trim_end_matches('\n').trim_end_matches('\r');

        match classify_header_line(trimmed, new_types) {
            HeaderAction::AlreadyOk => {
                return Ok(false);
            }
            HeaderAction::Replace => {
                if dry_run {
                    return Ok(true);
                }
                // Write new types line instead of the old one.
                writeln!(&mut prefix, "{}", new_types)?;
                // Fall through to copy remainder.
            }
            HeaderAction::Splice => {
                if dry_run {
                    return Ok(true);
                }
                // Insert new types before this line.
                writeln!(&mut prefix, "{}", new_types)?;
                prefix.extend_from_slice(line_buf.as_bytes());
                // Fall through to copy remainder.
            }
            HeaderAction::Continue => {
                if !dry_run {
                    prefix.extend_from_slice(line_buf.as_bytes());
                }
                continue;
            }
        }
        break;
    }

    // Write prefix + remainder to temp file, then persist.
    let parent_dir = new_path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = NamedTempFile::new_in(parent_dir)
        .with_context(|| format!("creating temp file in {}", parent_dir.display()))?;

    tmp.write_all(&prefix)?;
    std::io::copy(&mut reader, &mut tmp)
        .with_context(|| format!("copying remainder of {}", path.display()))?;

    tmp.persist(new_path)
        .with_context(|| format!("persisting temp file to {}", new_path.display()))?;

    Ok(true)
}

pub fn read_types_file(path: &Path) -> Result<String> {
    let file = File::open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    let buf_read = BufReader::new(file);
    let mut lines = buf_read.lines();

    if let Some(line_result) = lines.next() {
        let line = line_result.with_context(|| format!("reading {}", path.display()))?;
        if line.starts_with("@Types:") {
            return Ok(line);
        } else {
            bail!("expected @Types: header in {}, got: {}", path.display(), line);
        }
    }
    bail!("expected @Types: header in {}, got empty file", path.display())
}

/// Walk base_path once, collecting type mappings and .cha file paths,
/// then update all .cha files. Returns paths of files actually changed.
pub fn update_types_in_place(base_path: &Path, dry_run: bool) -> Result<Vec<PathBuf>> {
    let mut types_dirs: HashSet<PathBuf> = HashSet::new();
    let mut types_map: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
    let mut cha_files: Vec<PathBuf> = Vec::new();

    // Single walk: collect types_map, types_dirs, and cha_files.
    for result_entry in WalkDir::new(base_path)
        .into_iter()
        .filter_entry(|e| e.file_name().to_str().map(|s| s != ".git").unwrap_or(false))
    {
        let entry = result_entry.context("walking directory")?;
        let file_type = entry.file_type();
        if file_type.is_dir() {
            let dir_path = entry.into_path();
            if dir_path == base_path {
                types_map.insert(dir_path, None);
            } else {
                let parent_dir_path = dir_path.parent().unwrap().to_path_buf();
                types_map.insert(dir_path, types_map.get(&parent_dir_path).unwrap().clone());
            }
        } else if file_type.is_file() {
            if entry.file_name() == "0types.txt" {
                let dir_path = entry.into_path().parent().unwrap().to_path_buf();
                types_dirs.insert(dir_path.clone());
                types_map.insert(dir_path.clone(), Some(dir_path));
            } else if entry.path().extension().is_some_and(|ext| ext == "cha") {
                cha_files.push(entry.into_path());
            }
        }
    }

    // Parse all the @Types files.
    let types_info: HashMap<PathBuf, String> = types_dirs
        .iter()
        .map(|dir| {
            let path_buf = dir.join("0types.txt");
            let new_types = read_types_file(&path_buf)?;
            Ok((dir.clone(), new_types))
        })
        .collect::<Result<_>>()?;

    // Process all .cha files.
    let mut updated_files: Vec<PathBuf> = Vec::new();
    for cha_path in &cha_files {
        if let Some(types_dir) = types_map.get(cha_path.parent().unwrap()).unwrap() {
            let new_types = types_info.get(types_dir).unwrap();
            let updated = update_types_to_new_path(cha_path, cha_path, new_types, dry_run)?;
            if updated {
                updated_files.push(cha_path.clone());
            }
        }
    }
    Ok(updated_files)
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("fixtures/small-types.cha", Some("@Types:\tlong, toyplay, TD"))]
    #[case("fixtures/big-types.cha", Some("@Types:\tlong, toyplay, TD"))]
    #[case("fixtures/tiny-types.cha", Some("@Types:\tlong, toyplay, TD"))]
    #[case("fixtures/no-types.cha", None)]
    fn test_get_types(#[case] filename: &str, #[case] expected: Option<&str>) {
        let path = Path::new(filename);
        assert_eq!(
            get_types(path).unwrap(),
            expected.map(|s| s.to_owned())
        );
    }

    #[rstest]
    #[case("@Types:\tlong, toyplay, TD", "@Types:\tlong, toyplay, TD", HeaderAction::AlreadyOk)]
    #[case("@Types:\tlong, toyplay, OLD", "@Types:\tlong, toyplay, TD", HeaderAction::Replace)]
    #[case("@End", "@Types:\tlong, toyplay, TD", HeaderAction::Splice)]
    #[case("*CHI:\thello.", "@Types:\tlong, toyplay, TD", HeaderAction::Splice)]
    #[case("@Begin", "@Types:\tlong, toyplay, TD", HeaderAction::Continue)]
    #[case("@Languages:\teng", "@Types:\tlong, toyplay, TD", HeaderAction::Continue)]
    fn test_classify_header_line(
        #[case] line: &str,
        #[case] new_types: &str,
        #[case] expected: HeaderAction,
    ) {
        assert_eq!(classify_header_line(line, new_types), expected);
    }

    #[test]
    fn test_read_types_file() {
        let path = Path::new("fixtures/test-dir/a/0types.txt");
        assert_eq!(read_types_file(path).unwrap(), "@Types:\tlong, toyplay, a");
    }

    #[test]
    fn test_read_types_file_nonexistent() {
        let path = Path::new("nonexistent/0types.txt");
        assert!(read_types_file(path).is_err());
    }
}
