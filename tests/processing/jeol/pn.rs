use super::*;
use nmr::processing::NusSettings;

// Physical P/N coherence paths: P=exp(i(w2*t2+w1*t1)),
// N=exp(i(w2*t2-w1*t1)). The on-disk pairs are Re(P),-Im(P),
// Re(N),-Im(N). This is deliberately not generated from the reader's
// Cartesian mapping. Both signs of w1 are present in the same acquisition.
fn fixture(indices: &[usize], declaration: &str) -> Vec<u8> {
    let original = jdf(3, [indices.len(), 32], [0.0; 2], |t1, t2| {
        let t1 = indices[(t1 * 32.0).round() as usize] as f64 / 32.0;
        let mut p = C::default();
        let mut n = C::default();
        for (f1, f2, amplitude) in [(-6.0, 4.0, 1.0), (10.0, -7.0, 0.7)] {
            let phase = C::from_polar(amplitude, 0.37);
            p += phase * wave(TAU * (f2 * t2 + f1 * t1));
            n += phase * wave(TAU * (f2 * t2 - f1 * t1));
        }
        [p.re, -p.im, n.re, -n.im]
    });
    let mut records = vec![];
    for (name, kind, value) in [
        ("pn_type", 0i32, declaration.as_bytes().to_vec()),
        ("y_orig_points", 1, 32i32.to_le_bytes().to_vec()),
        ("y_sweep", 2, 32f64.to_le_bytes().to_vec()),
    ] {
        let mut record = vec![0u8; 64];
        record[6] = 1;
        record[7] = if name == "y_sweep" { 13 } else { 0 };
        record[16..16 + value.len()].copy_from_slice(&value);
        record[32..36].copy_from_slice(&kind.to_le_bytes());
        record[36..36 + name.len()].copy_from_slice(name.as_bytes());
        records.extend(record);
    }
    let list_start = 1360 + 16 + records.len();
    let data_start = list_start + indices.len() * 8;
    let mut bytes = original[..1360].to_vec();
    for value in [64u32, 0, 3, 208] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(records);
    for &i in indices {
        bytes.extend((i as f64 / 32.0).to_be_bytes());
    }
    bytes.extend(&original[1360..]);
    for (offset, value) in [
        (1212, 1360),
        (1216, 208),
        (1284, data_start),
        (1224, list_start),
        (
            1256,
            if indices.len() < 32 {
                indices.len() * 8
            } else {
                0
            },
        ),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&(value as u32).to_be_bytes());
    }
    bytes
}

fn check_peaks(output: &nmr::Dataset) {
    let p = output.as_processed().unwrap();
    assert_eq!(p.descriptor().logical_shape(), vec![32, 32]);
    for (y, x, amplitude) in [(10, 20, 1.0), (26, 9, 0.7)] {
        let magnitude = |row| {
            (0..2)
                .flat_map(|a| (0..2).map(move |b| (a, b)))
                .map(|(a, b)| p.data().get(&[row, x], &[a, b]).unwrap().powi(2))
                .sum::<f64>()
                .sqrt()
        };
        let peak = magnitude(y);
        assert!(
            (peak / (1024.0 * amplitude) - 1.0).abs() < 1e-4,
            "peak={peak}"
        );
        assert!(magnitude(32 - y) / peak < 1e-5, "mirror survived");
    }
}

#[test]
fn pn_y_decodes_before_dense_fft_and_sparse_ist_on_both_sides_of_carrier() {
    for indices in [
        (0..32).collect::<Vec<_>>(),
        (0..20).map(|i| i * 7 % 32).collect(),
    ] {
        let bytes = fixture(&indices, "y");
        let input = read(&bytes);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pn.jdf");
        std::fs::write(&path, &bytes).unwrap();
        let disk = nmr::ReadOptions::new()
            .allow_experimental_vendor_semantics(true)
            .read(&path)
            .unwrap();
        assert_eq!(
            input.as_raw().unwrap().descriptor(),
            disk.as_raw().unwrap().descriptor()
        );
        for &i in &indices {
            assert_eq!(
                input
                    .as_raw()
                    .unwrap()
                    .data()
                    .read_trace(&[i])
                    .unwrap()
                    .samples(),
                disk.as_raw()
                    .unwrap()
                    .data()
                    .read_trace(&[i])
                    .unwrap()
                    .samples()
            );
        }
        let input = snapshot(&input);
        if indices.len() == 32 {
            for order in [[0, 1], [1, 0]] {
                check_peaks(&apply(
                    &input,
                    vec![fft(order[0], Sign::Negative), fft(order[1], Sign::Negative)],
                ));
            }
        }
        let mixed = NusSettings {
            max_iterations: 1000,
            noise_standard_deviation: Some(0.0),
        }
        .prepare(
            &input,
            ProcessingPlan::new(vec![fft(1, Sign::Negative)]).unwrap(),
            ProcessingOptions::new(),
        )
        .unwrap()
        .execute()
        .unwrap();
        check_peaks(&apply(&snapshot(&mixed), vec![fft(0, Sign::Negative)]));
    }
}

#[test]
fn unsupported_pn_declaration_is_not_silently_cartesian() {
    for declaration in ["x", "z", "unknown"] {
        let bytes = fixture(&(0..32).collect::<Vec<_>>(), declaration);
        assert!(read_parts(Parts::new(&bytes).allow_experimental_vendor_semantics(true)).is_err());
    }
}
