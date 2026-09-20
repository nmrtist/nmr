use crate::allocations::{ALLOCATIONS, COUNTING};
use std::{hint::black_box, sync::atomic::Ordering, time::Instant};

fn timed(mut run: impl FnMut() -> f64) -> (f64, f64) {
    for _ in 0..2 {
        black_box(run());
    }
    let start = Instant::now();
    let mut sum = 0.0;
    for _ in 0..10 {
        sum = black_box(run());
    }
    (start.elapsed().as_secs_f64() * 100.0, sum)
}
pub(crate) fn run() {
    let n = 2_097_152;
    let data = nmr::processed::ProcessedData::new(
        vec![n],
        vec![1],
        (0..n).map(|i| (i % 101) as f64).collect(),
    )
    .unwrap();
    // Compare coordinate-based access with component-plane and slice iteration.
    let (old, sum) = timed(|| {
        (0..n)
            .map(|i| {
                let coordinate = black_box(vec![i]);
                data.get(&coordinate, &[0]).unwrap()
            })
            .sum()
    });
    let (plane, actual) = timed(|| data.component_plane(&[0]).unwrap().copied().sum());
    let (slice, expected) = timed(|| data.samples().iter().copied().sum());
    assert_eq!(sum, actual);
    assert_eq!(actual, expected);
    for data in [
        &data,
        &nmr::processed::ProcessedData::new(
            vec![12, 19, 7],
            vec![2, 3, 2],
            vec![1.0; 12 * 19 * 7 * 12],
        )
        .unwrap(),
    ] {
        let components = vec![0; data.shape().len()];
        let iter = data.component_plane(&components).unwrap();
        ALLOCATIONS.store(0, Ordering::Relaxed);
        COUNTING.store(true, Ordering::Relaxed);
        black_box(iter.copied().sum::<f64>());
        COUNTING.store(false, Ordering::Relaxed);
        assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0);
    }
    println!(
        "release scalar sum n={n}, mean of 10 after 2 warmups: coordinate_access_ms={old:.6}, plane_ms={plane:.6}, slice_ms={slice:.6}, sum={sum}; scalar and multidimensional iterator allocations=0"
    );
}
