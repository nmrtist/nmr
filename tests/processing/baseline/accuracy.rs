//! Independent analytic release gates. Expected signals use closed-form
//! physical line shapes or tones, never the processing kernels under test.

use nmr::processing::PositivePeaksV1;

fn integral(coordinates: &[f64], samples: &[f64], low: f64, high: f64) -> f64 {
    coordinates
        .windows(2)
        .zip(samples.windows(2))
        .filter(|(x, _)| x[0] >= low && x[1] <= high)
        .map(|(x, y)| (x[1] - x[0]) * (y[0] + y[1]) / 2.0)
        .sum()
}

#[test]
fn asls_preserves_narrow_peak_height_area_and_line_shape_when_resampled() {
    let mut density_references: [Option<Vec<f64>>; 2] = [None, None];
    for points in [129, 513, 2049, 8193] {
        for (width_index, width) in [22.0_f64, 44.0].into_iter().enumerate() {
            let coordinates: Vec<_> = (0..points)
                .map(|index| 1200.0 * index as f64 / (points - 1) as f64)
                .collect();
            let truth: Vec<_> = coordinates
                .iter()
                .map(|x| 20.0 * (-((x - 530.0) / width).powi(2)).exp())
                .collect();
            let samples: Vec<_> = coordinates
                .iter()
                .zip(&truth)
                .map(|(x, peak)| {
                    peak + 2.0 + 0.003 * x + 2.0 * (std::f64::consts::PI * x / 1200.0).sin()
                })
                .collect();
            let corrected = PositivePeaksV1.subtract(&coordinates, &samples).unwrap();
            let error = (corrected
                .iter()
                .zip(&truth)
                .map(|(actual, expected)| (actual - expected).powi(2))
                .sum::<f64>()
                / truth.iter().map(|value| value * value).sum::<f64>())
            .sqrt();
            let height = corrected.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let true_height = truth.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let area_ratio = integral(
                &coordinates,
                &corrected,
                530.0 - 4.0 * width,
                530.0 + 4.0 * width,
            ) / integral(
                &coordinates,
                &truth,
                530.0 - 4.0 * width,
                530.0 + 4.0 * width,
            );
            println!(
                "AsLS n={points} width={width}: height={height:.6}, area={area_ratio:.6}, L2={error:.6}"
            );
            if width_index == 0 {
                assert!((height / true_height - 1.0).abs() < 0.02);
                assert!((area_ratio - 1.0).abs() < 0.05);
                assert!(error < 0.05);
            }
            // Broad peaks test sampling invariance only. This fixed smoothing
            // profile does not promise quantitative recovery of these peaks.
            let shared_grid: Vec<_> = (0..129)
                .map(|index| corrected[index * (points - 1) / 128])
                .collect();
            if let Some(reference) = &density_references[width_index] {
                let discrepancy = shared_grid
                    .iter()
                    .zip(reference)
                    .map(|(actual, expected)| (actual - expected).abs())
                    .fold(0.0_f64, f64::max);
                println!("AsLS width={width}: shared-grid max delta={discrepancy:.6}");
                assert!(discrepancy < 0.03);
            } else {
                density_references[width_index] = Some(shared_grid);
            }
        }
    }
}
