use lazy_static::lazy_static;
use regex::Captures;
use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::read_to_string;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use walkdir::WalkDir;

static DEBUG: bool = false;

lazy_static! {
    static ref TYPES_REGEX: Regex = Regex::new(r"(?m)^@Types:.+$").unwrap();
}

/// Find all 0types.txt and return a set of directories that have them,
/// along with a map of each subdirectory to itself or None.
pub fn collect_chat_types(path: &str) -> (HashSet<PathBuf>, HashMap<PathBuf, Option<PathBuf>>) {
    let mut types_dirs: HashSet<PathBuf> = HashSet::new();

    // Map to which closest ancestor directory has 0types.txt, if any at all.
    let mut types_map: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();

    // Don't go into .git directories.
    // Rely on depth-first.
    for result_entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| e.file_name().to_str().map(|s| s != ".git").unwrap_or(false))
    {
        let entry = result_entry.unwrap();
        let file_type = entry.file_type();
        if file_type.is_dir() {
            let dir_path = entry.into_path();
            let dir_path_str = dir_path.to_str().unwrap();
            if dir_path_str == path {
                types_map.insert(dir_path, None);
            } else {
                // Inherit from the parent's (already filled in depth-first).
                let parent_dir_path = dir_path.parent().unwrap().to_path_buf();
                types_map.insert(dir_path, types_map.get(&parent_dir_path).unwrap().clone());
            }
        } else if file_type.is_file() {
            let file_name_str = entry.file_name().to_str().unwrap();
            if file_name_str == "0types.txt" {
                let dir_path = entry.into_path().parent().unwrap().to_path_buf();
                types_dirs.insert(dir_path.clone());
                types_map.insert(dir_path.clone(), Some(dir_path));
            }
        } else {
            // Skip symlink.
        }
    }
    (types_dirs, types_map)
}

/// Extract @Types header if any, by slurping in whole file.
pub fn get_types_slurp(path: &str) -> Option<String> {
    let contents = read_to_string(path).unwrap();
    TYPES_REGEX.find(&contents).map(|m| m.as_str().to_owned())
}

/// Extract @Types header if any, by checking one line at a time
/// and bailing out if seeing end of transcript or first utterance.
pub fn get_types_fast(path: &str) -> Option<String> {
    let file = File::open(path).unwrap();
    let buf_read = BufReader::new(file);
    for line in buf_read.lines() {
        let good_line = line.unwrap();
        match good_line.as_bytes() {
            [b'@', b'E', b'n', b'd', ..] | [b'*', ..] => return None,
            [b'@', b'T', b'y', b'p', b'e', b's', b':', ..] => return Some(good_line),
            _ => {
                // Continue searching.
            }
        }
    }
    None
}

/// Slurp in order to do a replace.
/// Return whether a replacement happened.
pub fn replace_types_slurp(path: &str, new_types: &str) -> bool {
    let contents = read_to_string(path).unwrap();
    if let Some(m) = TYPES_REGEX.find(&contents) {
        if m.as_str() != new_types {
            lazy_static! {
                static ref TYPES_REGEX: Regex = Regex::new(r"(?m)^@Types:.+$").unwrap();
            }
            let result = TYPES_REGEX.replace(&contents, new_types);
            if DEBUG {
                assert_eq!(
                    TYPES_REGEX.find(&result).map(|m| m.as_str()),
                    Some(new_types)
                );
            }
            true
        } else {
            false
        }
    } else {
        // No old types header, so splice in where it belongs.
        lazy_static! {
            static ref SPLICE_REGEX: Regex = Regex::new(r"(?m)^((?:\*|@End))").unwrap();
        }
        let result = SPLICE_REGEX.replace(&contents, |caps: &Captures| {
            format!("{}\n{}", new_types, &caps[1])
        });

        if DEBUG {
            assert_eq!(
                TYPES_REGEX.find(&result).map(|m| m.as_str()),
                Some(new_types)
            );
        }
        true
    }
}

/// Return whether there was an update. Optionally write out prefix that
/// was read.
pub fn updated_prefix<I, W>(lines: &mut I, new_types: &str, mut w: Option<&mut W>) -> bool
where
    I: Iterator<Item = String>,
    W: Write,
{
    let mut updated = false;

    while let Some(line) = lines.next() {
        match line.as_bytes() {
            [b'@', b'E', b'n', b'd', ..] | [b'*', ..] => {
                if let Some(w) = w {
                    writeln!(w, "{}", new_types).unwrap();
                    writeln!(w, "{}", line).unwrap();
                }
                updated = true;
                break;
            }
            [b'@', b'T', b'y', b'p', b'e', b's', b':', ..] => {
                if line != new_types {
                    if let Some(w) = w {
                        writeln!(w, "{}", new_types).unwrap();
                    }
                    updated = true;
                    break;
                } else {
                    if let Some(w) = w {
                        writeln!(w, "{}", line).unwrap();
                    }
                    updated = false;
                    break;
                }
            }
            _ => {
                // Continue searching.
                if let Some(ref mut w) = w {
                    writeln!(w, "{}", line).unwrap();
                }
            }
        }
    }

    updated
}

/// Return whether a replacement happened.
/// Go one line at a time.
pub fn replace_types_fast(path: &str, new_types: &str) -> bool {
    let file = File::open(path).unwrap();
    let buf_read = BufReader::new(file);
    let lines = buf_read.lines();
    let mut strings = lines.map(|line| line.unwrap());

    // Do nothing with the prefix because we are just simulating
    // the real situation where we write out the prefix to a file.
    let mut w = vec![];

    updated_prefix(&mut strings, new_types, Some(&mut w))
}

/// Update @Types header in file, if needed.
/// if dry_run, don't actually copy any data or write out to output.
/// Return whether there was a change.
pub fn update_types_to_output<W: Write>(
    path: &str,
    new_types: &str,
    out: &mut W,
    dry_run: bool,
) -> bool {
    let file = File::open(path).unwrap();
    let buf_read = BufReader::new(file);
    let lines = buf_read.lines();
    let mut strings = lines.map(|line| line.unwrap());

    // Save the prefix to write out in case there was an update.
    let mut w = vec![];

    let updated = updated_prefix(&mut strings, new_types, Some(&mut w));

    if !dry_run {
        if updated {
            // Write out the prefix to out, then copy the rest of the
            // lines from path to it.
            out.write_all(&w).unwrap();
            while let Some(line) = strings.next() {
                writeln!(out, "{}", line).unwrap();
            }
        }
    }
    return updated;
}

/// Write to a temporary file before moving to new_path.
pub fn update_types_to_new_path(
    path: &str,
    new_path: &str,
    new_types: &str,
    dry_run: bool,
) -> bool {
    let file = File::open(path).unwrap();
    let buf_read = BufReader::new(file);
    let lines = buf_read.lines();
    let mut strings = lines.map(|line| line.unwrap());

    if dry_run {
        // Don't even bother to save prefix.
        let updated = updated_prefix(&mut strings, new_types, None as Option<&mut Vec<u8>>);
        updated
    } else {
        // Save the prefix to write out in case there was an update.
        let mut prefix = vec![];

        let updated = updated_prefix(&mut strings, new_types, Some(&mut prefix));

        if updated {
            // Use temporary file to write everything out to.
            let mut named_temp_file = NamedTempFile::new().unwrap();
            named_temp_file.write_all(&prefix).unwrap();
            while let Some(line) = strings.next() {
                writeln!(named_temp_file, "{}", line).unwrap();
            }

            // Finally, persist to new_path, which could have been
            // the same as path.
            named_temp_file.persist(new_path).unwrap();
        }
        updated
    }
}

pub fn read_types_file(path: &str) -> String {
    let file = File::open(path).unwrap();
    let buf_read = BufReader::new(file);
    let lines = buf_read.lines();
    let mut strings = lines.map(|line| line.unwrap());

    if let Some(line) = strings.next() {
        if line.starts_with("@Types:") {
            return line;
        } else {
            panic!("Expected @Types: header, got: {}", line);
        }
    }
    panic!("Expected @Types: header, got nothing");
}

/// Collect all 0types.txt under base_path, the apply modifications
/// to all CHAT files as appropriate. Return number of files actually
/// changed.
pub fn update_types_in_place(base_path: &str, dry_run: bool) -> u32 {
    lazy_static! {
        static ref CHAT_FILE_EXTENSION: &'static OsStr = OsStr::new("cha");
    }

    let mut num_updated = 0;

    let (types_dirs, types_map) = collect_chat_types(base_path);

    // Parse all the @Types files.
    let types_info: HashMap<PathBuf, String> = types_dirs
        .iter()
        .map(|dir| {
            let path_buf = dir.join("0types.txt");
            let new_types = read_types_file(path_buf.to_str().unwrap());
            (dir.clone(), new_types)
        })
        .collect();

    // For each CHAT file, update the @Types header if necessary.
    for result_entry in WalkDir::new(base_path)
        .into_iter()
        .filter_entry(|e| e.file_name().to_str().map(|s| s != ".git").unwrap_or(false))
    {
        let entry = result_entry.unwrap();
        let path = entry.path();
        let file_type = entry.file_type();
        if file_type.is_file() && path.extension() == Some(&CHAT_FILE_EXTENSION) {
            if let Some(types_dir) = types_map.get(path.parent().unwrap()).unwrap() {
                let new_types = types_info.get(types_dir).unwrap();
                let path_str = path.to_str().unwrap();
                let updated = update_types_to_new_path(path_str, path_str, new_types, dry_run);
                if updated {
                    num_updated += 1;
                }
            }
        }
    }
    num_updated
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn get_types_when_exists() {
        let paths = ["small-types.cha", "big-types.cha"];
        let expected = Some("@Types:\tlong, toyplay, TD".to_owned());

        for path in paths.iter() {
            for get_types in [get_types_slurp, get_types_fast].iter() {
                assert_eq!(get_types(path), expected);
            }
        }
    }

    #[test]
    fn get_types_when_does_not_exist() {
        let paths = ["no-types.cha"];
        let expected = None;

        for path in paths.iter() {
            for get_types in [get_types_slurp, get_types_fast].iter() {
                assert_eq!(get_types(path), expected);
            }
        }
    }

    #[test]
    fn replace_types_when_exists() {
        let path = "tiny-types.cha";
        let new_types = "@Types:\tlong, toyplay, FOO";
        for replace_types in [replace_types_slurp, replace_types_fast].iter() {
            assert_eq!(replace_types(path, new_types), true);
        }
    }

    #[test]
    fn replace_types_when_does_not_exist() {
        let path = "no-types.cha";
        let new_types = "@Types:\tlong, toyplay, FOO";
        for replace_types in [replace_types_slurp, replace_types_fast].iter() {
            assert_eq!(replace_types(path, new_types), true);
        }
    }

    #[test]
    fn collect_chat_types_mixed() {
        let path = "test-dir";
        let expected = (
            [
                PathBuf::from("test-dir/a"),
                PathBuf::from("test-dir/b"),
                PathBuf::from("test-dir/b/c"),
            ]
            .into_iter()
            .collect(),
            [
                (PathBuf::from("test-dir"), None),
                (
                    PathBuf::from("test-dir/a"),
                    Some(PathBuf::from("test-dir/a")),
                ),
                (
                    PathBuf::from("test-dir/b"),
                    Some(PathBuf::from("test-dir/b")),
                ),
                (
                    PathBuf::from("test-dir/b/c"),
                    Some(PathBuf::from("test-dir/b/c")),
                ),
                (
                    PathBuf::from("test-dir/b/d"),
                    Some(PathBuf::from("test-dir/b")),
                ),
            ]
            .into_iter()
            .collect(),
        );

        assert_eq!(collect_chat_types(path), expected);
    }
}
