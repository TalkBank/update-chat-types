use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::fs::File;
use update_chat_types::{get_types_fast, get_types_slurp, replace_types_fast, replace_types_slurp};

fn get_types_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_types");

    let paths = ["small-types.cha", "big-types.cha"];
    let file_infos: Vec<_> = paths
        .iter()
        .map(|path| (path, File::open(path).unwrap().metadata().unwrap().len()))
        .collect();
    let implementations = [
        (
            get_types_slurp as for<'r> fn(&'r str) -> Option<String>,
            "slurp",
        ),
        (
            get_types_fast as for<'r> fn(&'r str) -> Option<String>,
            "fast",
        ),
    ];

    for (&path, size) in file_infos.iter() {
        group.throughput(Throughput::Bytes(*size));
        for (get_types, get_types_label) in implementations.iter() {
            group.bench_with_input(BenchmarkId::new(*get_types_label, path), path, |b, path| {
                b.iter(|| get_types(path))
            });
        }
    }
    group.finish();
}

fn replace_types_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("replace_types");

    let new_types = "@Types:\tlong, toyplay, FOO";
    let paths = [
        "no-types.cha",
        "tiny-types.cha",
        "small-types.cha",
        "big-types.cha",
    ];
    let file_infos: Vec<_> = paths
        .iter()
        .map(|path| (path, File::open(path).unwrap().metadata().unwrap().len()))
        .collect();
    let implementations = [
        (
            replace_types_slurp as for<'r> fn(&'r str, &str) -> bool,
            "slurp",
        ),
        (
            replace_types_fast as for<'r> fn(&'r str, &str) -> bool,
            "fast",
        ),
    ];

    for (&path, size) in file_infos.iter() {
        group.throughput(Throughput::Bytes(*size));
        for (replace_types, replace_types_label) in implementations.iter() {
            group.bench_with_input(
                BenchmarkId::new(*replace_types_label, path),
                path,
                |b, path| b.iter(|| replace_types(path, new_types)),
            );
        }
    }
    group.finish();
}

criterion_group!(benches, get_types_benchmark, replace_types_benchmark);
criterion_main!(benches);
