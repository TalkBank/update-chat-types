use divan::{Bencher, counter::BytesCount};
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use update_chat_types::{get_types, update_types_to_new_path};

fn main() {
    divan::main();
}

const GET_TYPES_PATHS: &[&str] = &["fixtures/small-types.cha", "fixtures/big-types.cha"];

#[divan::bench(args = GET_TYPES_PATHS)]
fn get_types_fast(bencher: Bencher, path: &str) {
    let path = Path::new(path);
    let size = fs::metadata(path).unwrap().len();
    bencher
        .counter(BytesCount::from(size))
        .bench(|| get_types(path).unwrap());
}

const UPDATE_TYPES_PATHS: &[&str] = &[
    "fixtures/no-types.cha",
    "fixtures/tiny-types.cha",
    "fixtures/small-types.cha",
    "fixtures/big-types.cha",
];

#[divan::bench(args = UPDATE_TYPES_PATHS)]
fn update_types(bencher: Bencher, fixture: &str) {
    let fixture_path = Path::new(fixture);
    let size = fs::metadata(fixture_path).unwrap().len();
    let new_types = "@Types:\tlong, toyplay, FOO";
    bencher
        .counter(BytesCount::from(size))
        .with_inputs(|| {
            let tmp = TempDir::new().unwrap();
            let dst = tmp.path().join(fixture_path.file_name().unwrap());
            fs::copy(fixture_path, &dst).unwrap();
            (tmp, dst)
        })
        .bench_values(|(_tmp, dst)| {
            update_types_to_new_path(&dst, &dst, new_types, false).unwrap();
        });
}
