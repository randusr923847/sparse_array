use sparse_array::*;

#[test]
fn one_bucket() {
    let mut arr: SparseArray<u32> = SparseArray::with_capacity(64);
    assert!(!arr.has(5));
    assert!(arr.set(5, 123u32));
    assert!(arr.has(5));
    assert_eq!(arr.get(5), Some(&123u32));

    assert!(arr.set(0, 50u32));
    assert!(arr.set(63, 100u32));
    assert!(!arr.set(100, 100u32));

    assert_eq!(arr.get(5), Some(&123u32));
    assert_eq!(arr.get(63), Some(&100u32));
    assert_eq!(arr.get(0), Some(&50u32));
    assert_eq!(arr.get(50), None);

    assert!(arr.has(0));
    assert!(arr.has(63));
    assert!(!arr.has(6));

    assert!(arr.set(5, 456u32));
    assert_eq!(arr.get(5), Some(&456u32));

    let mut_ref = arr.get_mut(0);
    assert_eq!(mut_ref, Some(50u32).as_mut());

    (*mut_ref.unwrap()) = 75u32;
    assert_eq!(arr.get(0), Some(&75u32));
}

#[test]
fn many_buckets() {
  const N: usize = 20000;
  let mut arr: SparseArray<u32> = SparseArray::with_capacity(N);

  assert!(arr.set(5, 123u32));
  assert!(arr.set(1500, 456u32));
  assert!(arr.set(9876, 789u32));
  assert!(arr.set(19999, 1111u32));

  assert_eq!(arr.get(5), Some(&123u32));
  assert_eq!(arr.get(1500), Some(&456u32));
  assert_eq!(arr.get(9876), Some(&789u32));
  assert_eq!(arr.get(19999), Some(&1111u32));
  assert_eq!(arr.get(5000), None);

  assert!(arr.has(5));
  assert!(arr.has(1500));
  assert!(arr.has(9876));
  assert!(arr.has(19999));
  assert!(!arr.has(0));
  assert!(!arr.has(1234));

  assert!(arr.set(1500, 654u32));
  assert_eq!(arr.get(1500), Some(&654u32));

  let mut_ref = arr.get_mut(5);
  assert_eq!(mut_ref, Some(123u32).as_mut());

  (*mut_ref.unwrap()) = 75u32;
  assert_eq!(arr.get(5), Some(&75u32));

  assert_eq!(arr.get_mut(100), None);
}

#[test]
fn complex_type() {
  const N: usize = 128;
  let mut arr: SparseArray<Vec<u16>> = SparseArray::with_capacity(N);

  assert!(arr.set(5, vec![5; 5]));
  assert!(arr.set(100, vec![10; 10]));

  assert_eq!(arr.get(5), Some(vec![5; 5]).as_ref());
  assert_eq!(arr.get(100), Some(vec![10; 10]).as_ref());
}

#[test]
fn test_bitcode_pack() {
  const N: usize = 128;
  let mut arr: SparseArray<Vec<u16>> = SparseArray::with_capacity(N);

  assert!(arr.set(5, vec![5; 5]));
  assert!(arr.set(64, vec![7; 7]));
  assert!(arr.set(100, vec![10; 10]));

  arr.pack();

  assert_eq!(arr.get(5), Some(vec![5; 5]).as_ref());
  assert_eq!(arr.get(64), Some(vec![7; 7]).as_ref());
  assert_eq!(arr.get(100), Some(vec![10; 10]).as_ref());

  let encoded: Vec<u8> = bitcode::encode(&arr);
  let mut decoded: SparseArray<Vec<u16>> = bitcode::decode(&encoded).unwrap();

  assert_eq!(decoded.get(5), Some(vec![5; 5]).as_ref());
  assert_eq!(decoded.get(100), Some(vec![10; 10]).as_ref());

  assert!(!decoded.set(5, vec![3; 3]));
  assert!(!decoded.set(60, vec![7; 7]));

  let mut_ref = decoded.get_mut(100);
  assert_eq!(mut_ref, Some(vec![10; 10]).as_mut());
  assert_ne!(mut_ref, Some(vec![5; 10]).as_mut());

  (*mut_ref.unwrap()) = vec![8; 8];
  assert_eq!(decoded.get(100), Some(vec![8; 8]).as_ref());
}

#[test]
fn test_bitcode_unpack() {
  const N: usize = 128;
  let mut arr: SparseArray<Vec<u16>> = SparseArray::with_capacity(N);

  assert!(arr.set(5, vec![5; 5]));
  assert!(arr.set(100, vec![10; 10]));

  arr.pack();

  assert_eq!(arr.get(5), Some(vec![5; 5]).as_ref());
  assert_eq!(arr.get(100), Some(vec![10; 10]).as_ref());

  let encoded: Vec<u8> = bitcode::encode(&arr);
  let mut decoded: SparseArray<Vec<u16>> = bitcode::decode(&encoded).unwrap();

  decoded.unpack();

  assert_eq!(decoded.get(5), Some(vec![5; 5]).as_ref());
  assert_eq!(decoded.get(100), Some(vec![10; 10]).as_ref());

  assert!(decoded.set(5, vec![3; 3]));
  assert_eq!(decoded.get(5), Some(vec![3; 3]).as_ref());

  assert!(decoded.set(60, vec![7; 7]));
  assert_eq!(decoded.get(60), Some(vec![7; 7]).as_ref());

  let mut_ref = decoded.get_mut(100);
  assert_eq!(mut_ref, Some(vec![10; 10]).as_mut());
  assert_ne!(mut_ref, Some(vec![5; 10]).as_mut());

  (*mut_ref.unwrap()) = vec![8; 8];
  assert_eq!(decoded.get(100), Some(vec![8; 8]).as_ref());
}

#[test]
fn test_large() {
  const M: usize = 64 * 64 * 64 * 64;
  const N: usize = M * 64;
  let mut arr: SparseArray<Vec<usize>> = SparseArray::with_capacity(N);

  let mut i = 2;

  while i < N {
    assert!(arr.set(i, vec![i; 3]));
    i += M;
  }

  i = 1;

  while i < N {
    assert!(arr.set(i, vec![i; 6]));
    i += M;
  }

  let mut i = 2;

  while i < N {
    assert_eq!(arr.get(i), Some(vec![i; 3]).as_ref());
    i += M;
  }

  i = 1;

  while i < N {
    assert_eq!(arr.get(i), Some(vec![i; 6]).as_ref());
    i += M;
  }

  arr.pack();

  let encoded: Vec<u8> = bitcode::encode(&arr);
  drop(arr);
  let mut decoded: SparseArray<Vec<usize>> = bitcode::decode(&encoded).unwrap();

  let mut i = 2;

  while i < N {
    assert_eq!(decoded.get(i), Some(vec![i; 3]).as_ref());
    i += M;
  }

  i = 1;

  while i < N {
    assert_eq!(decoded.get(i), Some(vec![i; 6]).as_ref());
    i += M;
  }
}
