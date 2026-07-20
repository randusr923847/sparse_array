# Sparse Array

A rust sparse array (map) implementation which, for large + sparse data, is faster than `HashMap`,
with less memory usage than `Vec`.

Efficiently maps `usize` to any data type. Based on [Google sparsetable](https://github.com/sparsehash/sparsehash/blob/master/src/sparsehash/sparsetable), with ~2 bits of overhead
per slot (see [Smerity's article](https://smerity.com/articles/2015/google_sparsehash.html) for more details).

This data structure is most appropriate when the range of indices is large, only a small percentage of those
indices are used/occupied, and the indices which are used are spread out within the range of indices.

The capacity (range of indices) must be set at constructor time. Currently, this implementation is designed
for use cases where the array/map is created fully first, then the map can be stored and used at a later time
for fast retrieval. Resizing or removal features have not been implemented yet.

The `bitcode` feature flag enables encoding/decoding (for storage) using the [bitcode](https://github.com/softbearstudios/bitcode) crate.
For storage/portability, the array must be "packed" first, see example and docs.

# Examples

```rust
use sparse_array::SparseArray;

let n = 10_000;
let mut arr: SparseArray<String> = SparseArray::with_capacity(n);

arr.set(5, String::from("five"));
arr.set(1234, String::from("one thousand two hundred thirty four"));
arr.set(9999, String::from("nine thousand nine hundred ninety nine"));

let success = arr.set(20000, String::from("should fail"));
assert_eq!(success, false);

assert!(arr.has(1234));
assert!(!arr.has(2000));

assert_eq!(arr.get(5).unwrap(), "five");
assert_eq!(arr.get(6), None);

arr.get_mut(5).unwrap().push_str("!");
assert_eq!(arr.get(5).unwrap(), "five!");

// pack the array into portable format for encoding & storage
// when in packed form, no new insertions can be made (for performance)
arr.pack();

assert_eq!(arr.get(1234).unwrap(), "one thousand two hundred thirty four");

// set() cannot be used in packed form
assert_eq!(arr.set(6, String::from("six")), false);

// existing elements can be modified using get_mut()
let s = arr.get_mut(5).unwrap();
s.clear();
s.push_str("5");

assert_eq!(arr.get(5).unwrap(), "5");

// the array can be unpacked to restore insertion ability
arr.unpack();

assert_eq!(arr.set(6, String::from("six")), true);
assert_eq!(arr.get(6).unwrap(), "six");
```