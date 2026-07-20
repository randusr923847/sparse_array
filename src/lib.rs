//! A sparse array (map) implementation which, for large + sparse data, is faster than `HashMap`,
//! with less memory usage than `Vec`.
//!
//! Efficiently maps `usize` to any data type. Based on [Google sparsetable][google], with ~2 bits of overhead
//! per slot (see [Smerity's article][smerity] for more details).
//!
//! This data structure is most appropriate when the range of indices is large, only a small percentage of those
//! indices are used/occupied, and the indices which are used are spread out within the range of indices.
//!
//! The capacity (range of indices) must be set at constructor time. Currently, this implementation is designed
//! for use cases where the array/map is created fully first, then the map can be stored and used at a later time
//! for fast retrieval. Resizing or removal features have not been implemented yet.
//!
//! The `bitcode` feature flag enables encoding/decoding (for storage) using the [bitcode] crate.
//! For storage/portability, the array must be ["packed"] first, see example and docs.
//!
//! [google]: https://github.com/sparsehash/sparsehash/blob/master/src/sparsehash/sparsetable
//! [smerity]: https://smerity.com/articles/2015/google_sparsehash.html
//! [bitcode]: https://github.com/softbearstudios/bitcode
//! ["packed"]: SparseArray::pack
//!
//! # Examples
//!
//! ```rust
//! use sparse_array::SparseArray;
//!
//! let n = 10_000;
//! let mut arr: SparseArray<String> = SparseArray::with_capacity(n);
//!
//! arr.set(5, String::from("five"));
//! arr.set(1234, String::from("one thousand two hundred thirty four"));
//! arr.set(9999, String::from("nine thousand nine hundred ninety nine"));
//!
//! let success = arr.set(20000, String::from("should fail"));
//! assert_eq!(success, false);
//!
//! assert!(arr.has(1234));
//! assert!(!arr.has(2000));
//!
//! assert_eq!(arr.get(5).unwrap(), "five");
//! assert_eq!(arr.get(6), None);
//!
//! arr.get_mut(5).unwrap().push_str("!");
//! assert_eq!(arr.get(5).unwrap(), "five!");
//!
//! // pack the array into portable format for encoding & storage
//! // when in packed form, no new insertions can be made (for performance)
//! arr.pack();
//!
//! assert_eq!(arr.get(1234).unwrap(), "one thousand two hundred thirty four");
//!
//! // set() cannot be used in packed form
//! assert_eq!(arr.set(6, String::from("six")), false);
//!
//! // existing elements can be modified using get_mut()
//! let s = arr.get_mut(5).unwrap();
//! s.clear();
//! s.push_str("5");
//!
//! assert_eq!(arr.get(5).unwrap(), "5");
//!
//! // the array can be unpacked to restore insertion ability
//! arr.unpack();
//!
//! assert_eq!(arr.set(6, String::from("six")), true);
//! assert_eq!(arr.get(6).unwrap(), "six");
//! ```

use std::mem::size_of;
use std::alloc::{alloc, dealloc, handle_alloc_error, Layout};

#[cfg(feature = "bitcode")]
use bitcode::{Encode, Decode};

// # Terminology:
// N            = total capacity of array
// M            = number of buckets (N.div_ceil(GROUP_SIZE))
// `GROUP_SIZE` = number of elements in each bucket

// element - each individual item in the array
// bucket  - each group of GROUP_SIZE elements

// `BucketData`  : (bitmap, pointer)
// `buckets`     : Vec<BucketData>
// `bitmap`      : usize    - storing existence bits for each element
// `pointer`     : usize    - storing either pointer to bucket data or index into data vec (if packed)
// `data`        : Vec<T>   - storing raw data sequentially, only used in packed form

// in the code:
// `index`    is always the logical index of the element in the abstract super array
// `bm_ind`   is always the index of a bucket in `buckets`
// `bit_ind`  is always the index of a bit within a bucket
// `item_ind` is always the index of an element in the data bucket vec (actual, not logical)

// Google implementation uses 32x + 16 for GROUP_SIZE
// for simplicity here, using same as pointer size
// TBD: benchmark perf diff with changes to GROUP_SIZE and update
const GROUP_SIZE: usize = size_of::<usize>();

#[derive(Clone, Debug)]
#[cfg_attr(feature = "bitcode", derive(Encode, Decode))]
struct BucketData {
  bitmap: usize,
  pointer: usize
}

// n is the total logical capacity of the array
// m is the number of buckets
/// The sparse array type.
#[derive(Debug)]
#[cfg_attr(feature = "bitcode", derive(Encode, Decode))]
pub struct SparseArray<T: Clone> {
  buckets: Vec<BucketData>,
  data: Vec<T>,
  n: usize,
}

impl<T: Clone> SparseArray<T> {
  /// Identical to [`with_capacity`], see below.
  ///
  /// [`with_capacity`]: SparseArray::with_capacity
  #[inline]
  #[must_use]
  pub fn new(n: usize) -> Self {
    SparseArray::<T>::with_capacity(n)
  }

  /// Constructs new empty [`SparseArray`] with specified capacity.
  ///
  /// Importantly here, capacity should be based on the range of the indices being mapped rather than
  /// the total number of elements expected to be stored in the array.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use sparse_array::SparseArray;
  /// let mut arr: SparseArray<String> = SparseArray::with_capacity(10_000);
  /// ```
  ///
  /// [`SparseArray`]: SparseArray
  #[inline]
  #[must_use]
  pub fn with_capacity(n: usize) -> Self {
    let m: usize = n.div_ceil(GROUP_SIZE);

    Self {
      buckets: vec![BucketData { bitmap: 0, pointer: usize::MAX }; m],
      data: Vec::new(),
      n: n,
    }
  }

  // allocate array
  #[inline(always)]
  fn new_arr(len: usize) -> *mut T {
    unsafe {
      let layout = Layout::array::<T>(len).unwrap();

      let raw = alloc(layout);
      if raw.is_null() {
          handle_alloc_error(layout);
      }

      raw as *mut T
    }
  }

  // copy arr values
  // no validity checks!!!
  // no deallocations!!!
  // dst must be empty
  // src must be deallocated by caller
  // shouldn't be overlapping
  #[inline(always)]
  fn copy_arr_vals(src: *mut T, dst: *mut T, count: usize) {
    if std::mem::needs_drop::<T>() {
      for i in 0..count {
        unsafe {
          dst.add(i).write((*src.add(i)).clone());
        }
      }
    }
    // if type allows copying, directly memcpy values
    else {
      unsafe {
        std::ptr::copy_nonoverlapping(src, dst, count);
      }
    }
  }

  // resize and insert value, copying values over and shifting right
  // deallocs old arr
  #[inline(always)]
  fn insert_in_arr(ptr: *mut T, ind: usize, val: T, curr_len: usize) -> *mut T {
    let new_ptr = SparseArray::<T>::new_arr(curr_len + 1);
    SparseArray::<T>::copy_arr_vals(ptr, new_ptr, ind);

    unsafe {
      new_ptr.add(ind).write(val);

      SparseArray::<T>::copy_arr_vals(ptr.add(ind), new_ptr.add(ind + 1), curr_len - ind);

      let layout = Layout::array::<T>(curr_len).unwrap();
      dealloc(ptr as *mut u8, layout);
    }

    new_ptr
  }

  // check if a bit in the bitmap is set
  // bm_ind is the index of the bucket in the bitmap
  // bit_ind is the index of the bit within that bucket
  #[inline(always)]
  fn is_set(bm: usize, bit_ind: usize) -> bool {
    let mask: usize = 1 << (GROUP_SIZE - 1 - bit_ind);
    mask & bm > 0
  }

  // set bit in bitmap to 1
  #[inline(always)]
  fn set_bit(&mut self, bm_ind: usize, bit_ind: usize) {
    let mask: usize = 1 << (GROUP_SIZE - 1 - bit_ind);
    self.buckets[bm_ind].bitmap = self.buckets[bm_ind].bitmap | mask;
  }

  // wrapper for is_set
  #[inline(always)]
  fn _has(bm: usize, bit_ind: usize) -> bool {
    SparseArray::<T>::is_set(bm, bit_ind)
  }

  // returns bucket size, counts ones in bucket bitmap
  #[inline(always)]
  fn get_bucket_size(bm: usize) -> usize {
    bm.count_ones() as usize
  }

  // returns the bucket and item inds for data retrieval or placement
  #[inline(always)]
  fn get_item_ind(bm: usize, bit_ind: usize) -> usize {
    if bit_ind == 0 {
      return 0;
    }

    // bits before the bit_ind
    // popcount (count_ones) of these bits gives item_ind
    let bits = bm >> (GROUP_SIZE - bit_ind);

    bits.count_ones() as usize
  }

  /// Checks if index is occupied in the array.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use sparse_array::SparseArray;
  ///
  /// let mut arr: SparseArray<u32> = SparseArray::with_capacity(10_000);
  ///
  /// assert_eq!(arr.has(10), false);
  /// assert_eq!(arr.set(10, 10), true);
  /// assert_eq!(arr.has(10), true);
  /// ```
  #[inline]
  pub fn has(&self, index: usize) -> bool {
    if index >= self.n {
      return false;
    }

    let bm_ind = index / GROUP_SIZE;
    let bit_ind = index % GROUP_SIZE;
    let bitmap = self.buckets[bm_ind].bitmap;

    SparseArray::<T>::is_set(bitmap, bit_ind)
  }

  /// Set the value at an index. Can't be used if array is in [packed] form.
  ///
  /// Returns `true` if succeeded, `false` if index is not valid or if array is in [packed] form.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use sparse_array::SparseArray;
  ///
  /// let mut arr: SparseArray<u32> = SparseArray::with_capacity(1_000);
  ///
  /// assert_eq!(arr.set(10, 10), true);
  /// assert_eq!(arr.get(10), Some(&10));
  ///
  /// assert_eq!(arr.set(2000, 20), false);
  ///
  /// // can be used to change a value
  /// assert_eq!(arr.set(10, 11), true);
  /// assert_eq!(arr.get(10), Some(&11));
  ///
  /// arr.pack();
  ///
  /// // fails when in packed form
  /// assert_eq!(arr.set(15, 15), false);
  /// ```
  ///
  /// [packed]: SparseArray::pack
  #[inline]
  pub fn set(&mut self, index: usize, value: T) -> bool {
    if index >= self.n || self.data.capacity() > 0 {
      return false;
    }

    let bm_ind = index / GROUP_SIZE;
    let bit_ind = index % GROUP_SIZE;

    let bucket = &mut self.buckets[bm_ind];
    let item_ind = SparseArray::<T>::get_item_ind(bucket.bitmap, bit_ind);

    // if already in array, no resize needed, just update
    if SparseArray::<T>::_has(bucket.bitmap, bit_ind) {
      unsafe {
        (bucket.pointer as *mut T).add(item_ind).write(value);
      }

      return true;
    }

    // if bucket has no items yet, allocate array
    if bucket.pointer == usize::MAX {
      // create data arr
      let ptr = SparseArray::<T>::new_arr(1);
      unsafe { ptr.write(value); }

      bucket.pointer = ptr as usize;
    }
    // otherwise, insert
    else {
      let bucket_len = SparseArray::<T>::get_bucket_size(bucket.bitmap);
      let ptr = SparseArray::<T>::insert_in_arr(bucket.pointer as *mut T, item_ind, value, bucket_len);

      bucket.pointer = ptr as usize;
    }

    self.set_bit(bm_ind, bit_ind);

    true
  }

  // get helper, returns bucket.pointer & item_ind
  #[inline(always)]
  fn _get(&mut self, index: usize) -> Option<(usize, usize)> {
    if index >= self.n {
      return None;
    }

    let bm_ind = index / GROUP_SIZE;
    let bit_ind = index % GROUP_SIZE;
    let bucket = &self.buckets[bm_ind];

    if !SparseArray::<T>::_has(bucket.bitmap, bit_ind) {
      return None;
    }

    let item_ind = SparseArray::<T>::get_item_ind(bucket.bitmap, bit_ind);

    Some((bucket.pointer, item_ind))
  }

  /// Returns a reference to element if it exists in the array.
  ///
  /// Returns `None` if out of bounds or no value set for the index.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use sparse_array::SparseArray;
  ///
  /// let mut arr: SparseArray<String> = SparseArray::with_capacity(10_000);
  ///
  /// arr.set(10, String::from("ten"));
  /// assert_eq!(arr.get(10).unwrap(), "ten");
  ///
  /// assert_eq!(arr.get(15), None);
  /// ```
  #[inline]
  pub fn get(&mut self, index: usize) -> Option<&T> {
    match self._get(index) {
      None => {
        return None
      },
      Some((bucket_ptr, item_ind)) => {
        if self.data.capacity() > 0 {
          return Some( &self.data[bucket_ptr + item_ind] );
        }

        Some(unsafe { &*(bucket_ptr as *mut T).add(item_ind) })
      }
    }
  }

  /// Returns a mutable reference to element if it exists in the array.
  ///
  /// Returns `None` if out of bounds or no value set for the index.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use sparse_array::SparseArray;
  ///
  /// let mut arr: SparseArray<String> = SparseArray::with_capacity(10_000);
  ///
  /// arr.set(10, String::from("ten"));
  /// assert_eq!(arr.get_mut(10).unwrap(), "ten");
  ///
  /// assert_eq!(arr.get_mut(15), None);
  ///
  /// arr.get_mut(10).unwrap().push_str(" ten");
  /// assert_eq!(arr.get(10).unwrap(), "ten ten");
  /// ```
  #[inline]
  pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
    match self._get(index) {
      None => {
        return None
      },
      Some((bucket_ptr, item_ind)) => {
        if self.data.capacity() > 0 {
          return Some( &mut self.data[bucket_ptr + item_ind] );
        }

        Some(unsafe { &mut *(bucket_ptr as *mut T).add(item_ind) })
      }
    }
  }

  // pack the sparse array into portable format
  // move bucket data vecs sequentially into `data`
  // bucket data pointers changed to indices into `data` vec

  /// Packs the data allocations into the struct for portability.
  ///
  /// When packed, insertions would be significantly more expensive, so they are disallowed.
  /// The array can be [unpacked] to re-enable insertions.
  /// Intended to allow encoding and storage in the packed form. With the `bitcode` feature enabled,
  /// the `SparseArray` can be encoded with the [`bitcode`] crate.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use sparse_array::SparseArray;
  ///
  /// let mut arr: SparseArray<String> = SparseArray::with_capacity(10_000);
  /// arr.set(5000, String::from("hello world"));
  ///
  /// arr.pack();
  ///
  /// // now SparseArray can be safely encoded and stored
  /// // let encoded: Vec<u8> = bitcode::encode(&arr);
  /// // [write encoded to file]
  /// // [read encoded from file]
  /// // let mut decoded: SparseArray<String> = bitcode::decode(&encoded).unwrap();
  ///
  /// // has() and get() still work as expected
  /// assert_eq!(arr.has(5000), true);
  /// arr.get_mut(5000).unwrap().push_str("!");
  /// assert_eq!(arr.get(5000).unwrap(), "hello world!");
  ///
  /// // can't use set() on packed array
  /// assert_eq!(arr.set(0, String::from("fails")), false);
  /// ```
  ///
  /// [unpacked]: SparseArray::unpack
  /// [`bitcode`]: https://github.com/softbearstudios/bitcode
  #[inline]
  pub fn pack(&mut self) {
    for i in 0..self.buckets.len() {
      let ind = self.data.len();
      let bucket = &mut self.buckets[i];

      if bucket.pointer != usize::MAX {
        let bucket_len = SparseArray::<T>::get_bucket_size(bucket.bitmap);

        for j in 0..bucket_len {
          unsafe {
            self.data.push((*(bucket.pointer as *mut T).add(j)).clone());
          }
        }

        let layout = Layout::array::<T>(bucket_len).unwrap();
        unsafe { dealloc(bucket.pointer as *mut u8, layout); }

        bucket.pointer = ind;
      }
    }

    self.data.shrink_to_fit();
  }

  // unflatten data for more efficient insertion

  /// Unpacks the struct into data allocations which are more efficient for insertion.
  ///
  /// The goal of [packing] is to allow portability before the array/map is used. Generally, it is
  /// better to create and modify the map fully before packing, as unpacking adds the cost of allocating
  /// and copying values.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use sparse_array::SparseArray;
  ///
  /// let mut arr: SparseArray<String> = SparseArray::with_capacity(10_000);
  /// arr.set(5000, String::from("hello world"));
  ///
  /// arr.pack();
  ///
  /// // now SparseArray can be safely encoded and stored
  /// // let encoded: Vec<u8> = bitcode::encode(&arr);
  /// // [write encoded to file]
  /// // [read encoded from file]
  /// // let mut decoded: SparseArray<String> = bitcode::decode(&encoded).unwrap();
  /// // decoded.unpack();
  ///
  /// arr.unpack();
  ///
  /// // all array methods work as expected
  /// assert_eq!(arr.set(0, String::from("works")), true);
  /// ```
  ///
  /// [packing]: SparseArray::pack
  #[inline]
  pub fn unpack(&mut self) {
    for i in 0..self.buckets.len() {
      let bucket = &mut self.buckets[i];

      if bucket.pointer != usize::MAX {
        let bucket_len = SparseArray::<T>::get_bucket_size(bucket.bitmap);

        let ptr = SparseArray::<T>::new_arr(bucket_len);
        unsafe {
          SparseArray::<T>::copy_arr_vals((self.data.as_ptr() as *mut T).add(bucket.pointer), ptr, bucket_len);
        }
        bucket.pointer = ptr as usize;
      }
    }

    self.data = Vec::new();
  }
}

impl<T: Clone> Drop for SparseArray<T> {
  // need to free every allocation
  fn drop(&mut self) {
    if self.data.capacity() == 0 {
      for bucket in &self.buckets {
        if bucket.pointer != usize::MAX {
          let bucket_len = SparseArray::<T>::get_bucket_size(bucket.bitmap);
          let layout = Layout::array::<T>(bucket_len).unwrap();
          unsafe { dealloc(bucket.pointer as *mut u8, layout); }
        }
      }
    }
  }
}
