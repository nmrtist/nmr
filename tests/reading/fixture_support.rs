use nmr::Complex64;

pub(crate) fn sample_bits(samples: &[Complex64]) -> Vec<(u64, u64)> {
    samples
        .iter()
        .map(|sample| (sample.re.to_bits(), sample.im.to_bits()))
        .collect()
}

pub(crate) fn expected_bits(samples: &[(f64, f64)]) -> Vec<(u64, u64)> {
    samples
        .iter()
        .map(|&(real, imaginary)| (real.to_bits(), imaginary.to_bits()))
        .collect()
}
