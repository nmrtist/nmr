use nmr::Dataset;
use nmr::external::ExternalAxisSource;
use nmr::processed::{
    ComponentBasis, ProcessedDataset, ProcessedDescriptor, ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{
    FourierTransform, ProcessingOperation as Op, ProcessingOptions, ProcessingPlan,
};
use nmr::provenance::{InputAxisRef, InputSlot};

use crate::datasets::*;

#[test]
fn external_boundary_keeps_parent_evidence_and_restarts_state() {
    let fft = ProcessingPlan::new(vec![Op::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap()
    .apply(&raw())
    .unwrap();
    let descriptor = ProcessedDescriptor::new(vec![axis(4, ComponentBasis::Cartesian)]).unwrap();
    let boundary = fft
        .derive_external_processed(
            descriptor.clone(),
            vec![1.0; 8],
            vec![ExternalAxisSource::Parent(InputAxisRef::new(
                InputSlot::new(0),
                0,
            ))],
            declaration(),
        )
        .unwrap();
    let ProcessedOrigin::External(evidence) =
        boundary.as_processed().unwrap().provenance().origin()
    else {
        panic!("missing boundary")
    };
    assert_eq!(
        evidence.parent().canonical_digests(),
        fft.canonical_digests()
    );
    assert_eq!(
        evidence
            .parent()
            .provenance()
            .history()
            .unwrap()
            .records()
            .len(),
        1
    );
    assert!(
        boundary
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .is_none()
    );
    assert!(
        ProcessedDataset::from_dense_samples(
            descriptor.clone(),
            vec![2.0; 8],
            fft.as_processed().unwrap().provenance().clone()
        )
        .is_err()
    );
    assert!(
        fft.derive_external_processed(
            descriptor.clone(),
            vec![f64::NAN; 8],
            vec![ExternalAxisSource::New],
            declaration()
        )
        .is_err()
    );
    assert!(
        fft.derive_external_processed(
            descriptor,
            vec![1.0; 8],
            vec![ExternalAxisSource::Parent(InputAxisRef::new(
                InputSlot::new(0),
                8
            ))],
            declaration()
        )
        .is_err()
    );
    let result = reverse().apply(&boundary).unwrap();
    let restored = roundtrip(&result);
    assert_eq!(
        restored
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap()
            .records()
            .len(),
        1
    );
    assert!(
        restored
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap()
            .segments()[0]
            .accepted_archive()
    );
    let replay = result
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .unwrap()
        .replay(&[&boundary], ProcessingOptions::default())
        .unwrap();
    assert_eq!(replay.canonical_digests(), result.canonical_digests());
    assert_eq!(
        reverse().apply(&restored).unwrap().canonical_digests(),
        reverse().apply(&result).unwrap().canonical_digests()
    );
}

#[test]
fn external_axis_reordering_reduction_and_new_axes_are_checked() {
    let descriptor = ProcessedDescriptor::new(vec![
        axis(3, ComponentBasis::Scalar),
        axis(4, ComponentBasis::Scalar),
    ])
    .unwrap();
    let input: Dataset = ProcessedDataset::from_dense_samples(
        descriptor,
        vec![0.0; 12],
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap()
    .into();
    let parent = |axis| ExternalAxisSource::Parent(InputAxisRef::new(InputSlot::new(0), axis));
    for (axes, mapping, count) in [
        (
            vec![
                axis(2, ComponentBasis::Scalar),
                axis(3, ComponentBasis::Scalar),
            ],
            vec![parent(1), parent(0)],
            6,
        ),
        (vec![axis(2, ComponentBasis::Scalar)], vec![parent(1)], 2),
        (
            vec![axis(7, ComponentBasis::Scalar)],
            vec![ExternalAxisSource::New],
            7,
        ),
    ] {
        let output = input
            .derive_external_processed(
                ProcessedDescriptor::new(axes).unwrap(),
                vec![1.0; count],
                mapping,
                declaration(),
            )
            .unwrap();
        roundtrip(&output);
    }
    assert!(
        input
            .derive_external_processed(
                input.as_processed().unwrap().descriptor().clone(),
                vec![0.0; 12],
                vec![parent(0), parent(0)],
                declaration()
            )
            .is_err()
    );
    let imported = nmr::read(crate::fixture_paths::fixture("jcamp_dx/scaled-dif.dx")).unwrap();
    let boundary = imported
        .derive_external_processed(
            ProcessedDescriptor::new(vec![axis(2, ComponentBasis::Scalar)]).unwrap(),
            vec![0.0; 2],
            vec![parent(0)],
            declaration(),
        )
        .unwrap();
    let result = reverse().apply(&boundary).unwrap();
    let replay = result
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .unwrap()
        .replay(&[&boundary], ProcessingOptions::default())
        .unwrap();
    assert_eq!(result.canonical_digests(), replay.canonical_digests());
}
