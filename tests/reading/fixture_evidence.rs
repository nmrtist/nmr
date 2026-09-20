#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvidenceAuthority {
    PrimarySource,
    ExpertConfirmed,
    CorroboratedOracle,
    RegressionOnly,
}

#[derive(Clone, Copy)]
struct FixtureExpectation {
    name: &'static str,
    authority: EvidenceAuthority,
    opens_layout: bool,
}

const EXPECTATIONS: &[FixtureExpectation] = &[
    FixtureExpectation {
        name: "bruker/one-dimensional.fid",
        authority: EvidenceAuthority::PrimarySource,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "bruker/two-dimensional-kblock.ser",
        authority: EvidenceAuthority::ExpertConfirmed,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "bruker/two-dimensional-contiguous.ser",
        authority: EvidenceAuthority::ExpertConfirmed,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "jeol/axis-1.jdf",
        authority: EvidenceAuthority::CorroboratedOracle,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "jeol/axis-3.jdf",
        authority: EvidenceAuthority::CorroboratedOracle,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "jeol/axes-3-1.jdf",
        authority: EvidenceAuthority::PrimarySource,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "jeol/axes-3-3.jdf",
        authority: EvidenceAuthority::PrimarySource,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "jeol/axes-4-4.jdf",
        authority: EvidenceAuthority::PrimarySource,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "varian/complex.fid",
        authority: EvidenceAuthority::PrimarySource,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "varian/real.fid",
        authority: EvidenceAuthority::PrimarySource,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "varian/eight-trace.fid",
        authority: EvidenceAuthority::ExpertConfirmed,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "varian/two-block.fid",
        authority: EvidenceAuthority::PrimarySource,
        opens_layout: true,
    },
    FixtureExpectation {
        name: "npz/plot-v1.npz",
        authority: EvidenceAuthority::RegressionOnly,
        opens_layout: false,
    },
];

fn bruker_1d_parameters() -> &'static str {
    "##TITLE= fixture 1D\n\
     ##$TD= 4\n\
     ##$PARMODE= 0\n\
     ##$AQ_mod= 3\n\
     ##$BYTORDA= 0\n\
     ##$DTYPA= 0\n\
     ##$SW_h= 8000\n\
     ##$SFO1= 400.13\n\
     ##$BF1= 400.0\n\
     ##$O1= 1880\n\
     ##$NUC1= <1H>\n\
     ##END=\n"
}

fn varian_procpar(np: usize) -> String {
    format!("np 1 1 4 0 0 2 1 0 1 64\n1 {np}\n0\n")
}

#[test]
fn expectation_authority_never_uses_regression_only_to_open_a_layout() {
    assert!(EXPECTATIONS.iter().all(|expectation| {
        !expectation.opens_layout || expectation.authority != EvidenceAuthority::RegressionOnly
    }));
    assert!(
        EXPECTATIONS
            .iter()
            .all(|expectation| !expectation.name.is_empty())
    );
}

#[test]
fn vendor_metadata_snapshots_roundtrip_all_three_reader_families() {
    use nmr::snapshot::{self, AcceptRecordedHistory, SnapshotLimits};
    let bruker = nmr::formats::bruker::read_parts(nmr::formats::bruker::Parts::new(
        include_bytes!("../fixtures/bruker/one-dimensional.fid"),
        &[bruker_1d_parameters()],
    ))
    .unwrap();
    let jeol = nmr::formats::jeol::read_parts(
        nmr::formats::jeol::Parts::new(include_bytes!("../fixtures/jeol/axis-3.jdf"))
            .allow_experimental_vendor_semantics(true),
    )
    .unwrap();
    let varian = nmr::formats::varian::read_parts(nmr::formats::varian::Parts::new(
        include_bytes!("../fixtures/varian/complex.fid"),
        &varian_procpar(4),
    ))
    .unwrap();
    for raw in [bruker, jeol, varian] {
        let input = nmr::Dataset::from_raw(raw);
        let mut frame = Vec::new();
        snapshot::write_snapshot(&input, &mut frame, SnapshotLimits::default()).unwrap();
        let output = snapshot::read_snapshot(&mut frame.as_slice(), SnapshotLimits::default())
            .unwrap()
            .restore(AcceptRecordedHistory);
        assert_eq!(input.as_raw(), output.as_raw());
        assert_eq!(input.canonical_digests(), output.canonical_digests());
    }
}
