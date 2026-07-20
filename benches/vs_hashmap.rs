// adapted from https://github.com/Logicalshift/flo_sparse_array/blob/v0.1/benches/vs_hashmap.rs

use criterion::{criterion_group, criterion_main, Criterion};
use sparse_array::*;

use std::collections::{HashMap};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("store_hashmap_100k_to_self", |b| b.iter(|| {
        let mut array = HashMap::new();
        for p in 0..100000usize {
            array.insert(p * 2, p);
        }
    }));

    c.bench_function("store_sparse_array_100k_to_self", |b| b.iter(|| {
        let mut array = SparseArray::with_capacity(200000);

        for p in 0..100000usize {
            array.set(p * 2, p);
        }
    }));

    let mut hash_100k           = HashMap::new();
    let mut sparse_array_100k   = SparseArray::with_capacity(200000);

    for p in 0..100000usize {
        hash_100k.insert(p * 2, p);
        sparse_array_100k.set(p * 2, p);
    }

    c.bench_function("fetch_hashmap_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            assert!(hash_100k.get(&(p * 2)) == Some(&p));
        }
    }));

    c.bench_function("fetch_sparse_array_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            assert!(sparse_array_100k.get(p * 2) == Some(&p));
        }
    }));

    c.bench_function("insert_overwrite_hashmap_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            hash_100k.insert(p * 2, p);
        }
    }));

    c.bench_function("insert_overwrite_sparse_array_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            sparse_array_100k.set(p * 2, p);
        }
    }));

    c.bench_function("update_hashmap_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            (*hash_100k.get_mut(&(p * 2)).unwrap()) = p;
        }
    }));

    c.bench_function("update_sparse_array_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            (*sparse_array_100k.get_mut(p * 2).unwrap()) = p;
        }
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);