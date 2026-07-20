use std::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use bitcode::{Encode, Decode};

// sparse array/map based on Google sparsetable
// https://smerity.com/articles/2015/google_sparsehash.html
// https://github.com/sparsehash/sparsehash/blob/master/src/sparsehash/sparsetable

// goal: memory efficient, fast access sparse array
// goal: faster than hashmap, significantly less memory than vec

// N = total capacity of array
// M = number of buckets (N.div_ceil(GROUP_SIZE))
// GROUP_SIZE = number of elements in each bucket

// element - each individual item in the array
// bucket  - each group of GROUP_SIZE elements

// bitmap: [u64] storing existence bits for each element
// indices: [usize] storing index (<=> ptr) to bucket vec
// data: Vec<Vec<T>> storing the bucket vecs (ideally would be arrays)

// index is always the logical index of the element in the abstract super array
// bm_ind is always the index of a bucket
// bit_ind is always the index of a bit within a bucket
// bucket_ind is always the index of a data bucket vec
// item_ind is always the index of an element in the data bucket vec (actual, not logical)

const GROUP_SIZE: usize = 64;

#[derive(Encode, Decode, Clone)]
struct BucketData {
  bitmap: u64,
  pointer: usize
}

// N is the total logical capacity of the array
// M is the number of buckets
#[derive(Encode, Decode)]
pub struct SparseArray<T: Default + Clone> {
  buckets: Vec<BucketData>,
  pub data: Vec<T>,
  n: usize,
  m: usize,
}

impl<T: Default + Clone> SparseArray<T> {
  pub fn with_capacity(n: usize) -> Self {
    let m: usize = n.div_ceil(GROUP_SIZE);

    Self {
      buckets: vec![BucketData { bitmap: 0, pointer: usize::MAX }; m],
      data: Vec::new(),
      n: n,
      m: m,
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
  #[inline(always)]
  fn copy_arr_vals(src: *mut T, dst: *mut T, count: usize) {
    for i in 0..count {
      unsafe {
        dst.add(i).write((*src.add(i)).clone());
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
  fn is_set(bm: u64, bit_ind: usize) -> bool {
    let mask: u64 = 1 << (GROUP_SIZE - 1 - bit_ind);
    mask & bm > 0
  }

  // set bit in bitmap to 1
  #[inline(always)]
  fn set_bit(&mut self, bm_ind: usize, bit_ind: usize) {
    let mask: u64 = 1 << (GROUP_SIZE - 1 - bit_ind);
    self.buckets[bm_ind].bitmap = self.buckets[bm_ind].bitmap | mask;
  }

  // wrapper for is_set
  #[inline(always)]
  fn _has(bm: u64, bit_ind: usize) -> bool {
    SparseArray::<T>::is_set(bm, bit_ind)
  }

  // returns bucket size, counts ones in bucket bitmap
  #[inline(always)]
  fn get_bucket_size(bm: u64) -> usize {
    bm.count_ones() as usize
  }

  // returns the bucket and item inds for data retrieval or placement
  #[inline(always)]
  fn get_item_ind(bm: u64, bit_ind: usize) -> usize {
    if bit_ind == 0 {
      return 0;
    }

    // bits before the bit_ind
    // popcount (count_ones) of these bits gives item_ind
    let bits = bm >> (GROUP_SIZE - bit_ind);

    bits.count_ones() as usize
  }

  // public interface to check existence of element at index
  #[inline(always)]
  pub fn has(&self, index: usize) -> bool {
    if index >= self.n {
      return false;
    }

    let bm_ind = index / GROUP_SIZE;
    let bit_ind = index % GROUP_SIZE;
    let bitmap = self.buckets[bm_ind].bitmap;

    SparseArray::<T>::is_set(bitmap, bit_ind)
  }

  // set the value of an element
  #[inline(always)]
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

  // get the reference of an element
  #[inline(always)]
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

  // get the mutable reference of an element
  #[inline(always)]
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
  pub fn pack(&mut self) {
    for i in 0..self.m {
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
  pub fn unpack(&mut self) {
    for i in 0..self.m {
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

impl<T: Default + Clone> Drop for SparseArray<T> {
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
