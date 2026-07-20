// adapted from https://github.com/Logicalshift/flo_sparse_array/blob/v0.1/benches/vs_hashmap.rs

use criterion::{criterion_group, criterion_main, Criterion};
use sparse_array::*;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("store_vec_100k_to_self", |b| b.iter(|| {
        let mut array = vec![usize::MAX; 200000];
        for p in 0..100000usize {
            array[p * 2] = p;
        }
    }));

    c.bench_function("store_sparse_array_100k_to_self", |b| b.iter(|| {
        let mut array = SparseArray::with_capacity(200000);

        for p in 0..100000usize {
            array.set(p * 2, p);
        }
    }));

    let mut vec_100k            = vec![usize::MAX; 200000];
    let mut sparse_array_100k   = SparseArray::with_capacity(200000);

    for p in 0..100000usize {
        vec_100k[p * 2] = p;
        sparse_array_100k.set(p * 2, p);
    }

    c.bench_function("fetch_vec_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            assert!(vec_100k.get(p * 2) == Some(&p));
        }
    }));

    c.bench_function("fetch_sparse_array_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            assert!(sparse_array_100k.get(p * 2) == Some(&p));
        }
    }));

    c.bench_function("insert_overwrite_vec_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            vec_100k[p * 2] = p;
        }
    }));

    c.bench_function("insert_overwrite_sparse_array_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            sparse_array_100k.set(p * 2, p);
        }
    }));

    c.bench_function("update_vec_100k", |b| b.iter(|| {
        for p in 0..100000usize {
            (*vec_100k.get_mut(p * 2).unwrap()) = p;
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