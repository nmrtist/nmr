use crate::inputs;
use nmr::snapshot;

fn restored(input: &nmr::Dataset) -> nmr::Dataset {
    let mut bytes = Vec::new();
    snapshot::write_snapshot(input, &mut bytes, Default::default()).unwrap();
    snapshot::decode_snapshot(&bytes, Default::default())
        .unwrap()
        .restore(snapshot::AcceptRecordedHistory)
}

#[test]
fn jcamp_501_keeps_original_coordinates_scaling_and_strict_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.dx");
    for version in ["5.00", "5.01"] {
        let text = inputs::JCAMP.replace("5.00", version);
        std::fs::write(&path, &text).unwrap();
        let input = nmr::read(&path).unwrap();
        let processed = input.as_processed().unwrap();
        assert_eq!(processed.data().samples(), [2.0, 4.0, 6.0, 8.0]);
        assert_eq!(
            processed.descriptor().axes()[0]
                .coordinate_iter()
                .unwrap()
                .collect::<Vec<_>>(),
            [4.0, 3.0, 2.0, 1.0]
        );
        assert_eq!(
            processed
                .axis_evidence(0)
                .unwrap()
                .reference_frequency_mhz(),
            None
        );
        assert_eq!(
            restored(&input).canonical_digests(),
            input.canonical_digests()
        );
        for corrupt in [
            text.replace("NMR SPECTRUM", "LINK"),
            text.replace("NMR SPECTRUM", "NMR PEAK TABLE"),
            text.replace("##XYDATA", "##NTUPLES=NMR SPECTRUM\n##XYDATA"),
            text.replace("##XYDATA", "##DATA CLASS=XYPOINTS\n##XYDATA"),
            text.replace("##XYDATA", "##PEAK TABLE=(XY..XY)\n##XYDATA"),
            text.replace("##NPOINTS=4", "##NPOINTS=5"),
            text.replace("##NPOINTS=4", "##NPOINTS=4\n##NPOINTS=4"),
            text.replace("4 1 2 3 4", "4 1 2 3"),
            text.replace("##END=", ""),
            text.replace(version, "6.00"),
        ] {
            assert!(
                nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(
                    corrupt.as_bytes()
                ))
                .is_err(),
                "{corrupt}"
            );
        }
        let no_frequency = text
            .lines()
            .filter(|l| !l.starts_with("##.OBSERVE"))
            .collect::<Vec<_>>()
            .join("\n");
        let no_frequency = nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(
            no_frequency.as_bytes(),
        ))
        .unwrap();
        assert_eq!(
            no_frequency
                .axis_evidence(0)
                .unwrap()
                .reference_frequency_mhz(),
            None
        );
        assert_eq!(
            no_frequency
                .axis_evidence(0)
                .unwrap()
                .observe_frequency_mhz(),
            None
        );
    }
    // ASDF state/checkpoints and scaling use the same bounded decoder in 5.01.
    let dif = include_str!("../../fixtures/jcamp_dx/scaled-dif.dx").replace("5.00", "5.01");
    let dif =
        nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(dif.as_bytes()))
            .unwrap();
    assert_eq!(dif.data().samples(), [10.0, 11.0, 12.0, 13.0]);
}
