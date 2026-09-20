use nmr::formats::jcamp_dx::{Parts, read_parts, read_parts_with_limits};
use nmr::processing::{ProcessingError, ProcessingOperation, ProcessingOptions, ProcessingPlan};
use nmr::provenance::{ProcessedReadTransform, SourceDigest};
use nmr::{ReadErrorReason, ReadLimits, ReadOptions, ReadResource};
use sha2::{Digest, Sha256};

#[test]
fn path_and_parts_share_consumed_identity_scaling_and_strict_replay() {
    let text = include_str!("../../fixtures/jcamp_dx/scaled-dif.dx")
        .replace("\r\n", "\n")
        .replace("##YFACTOR=1", "##YFACTOR=2")
        .replace(
            "##XYDATA",
            "##UNKNOWN=first line\nsecond line\n$$ preserved comment\n##XYDATA",
        )
        .replace('\n', "\r\n");
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("synthetic.dx");
    std::fs::write(&path, &text).unwrap();
    let disk = nmr::read(&path).unwrap().into_processed().unwrap();
    let memory = read_parts(Parts::new(text.as_bytes())).unwrap();
    assert_eq!(
        disk.processed().canonical_digests(),
        memory.canonical_digests()
    );
    assert_eq!(memory.data().samples(), &[20.0, 22.0, 24.0, 26.0]);
    let source = &memory.provenance().sources()[0];
    assert!(source.locator().is_none());
    assert_eq!(source.id().unwrap().ordinal(), 0);
    assert_eq!(
        source.digest(),
        SourceDigest::Sha256(Sha256::digest(text.as_bytes()).into())
    );
    assert_eq!(
        disk.processed().provenance().read_record(),
        memory.provenance().read_record()
    );
    let record = memory.provenance().read_record().unwrap();
    assert_eq!(
        record.transform(),
        &ProcessedReadTransform::JcampDx {
            x_factor: 0.1,
            y_factor: 2.0
        }
    );
    assert_eq!(record.output_digests(), memory.canonical_digests());
    let ranges = memory
        .provenance()
        .source_metadata()
        .jcamp_dx()
        .unwrap()
        .records();
    assert_eq!(ranges.len(), 2);
    for range in ranges {
        assert_eq!(
            range.text().as_bytes(),
            &text.as_bytes()[range.byte_offset()..range.byte_offset() + range.text().len()]
        );
        assert_eq!(range.source(), source.id().unwrap());
    }
    assert!(
        ranges[0]
            .text()
            .contains("##UNKNOWN=first line\r\nsecond line\r\n$$ preserved comment\r\n")
    );
    assert!(!ranges[0].text().contains("1000 A0J"));
    assert_eq!(ranges[1].text(), "##END=\r\n");
    let metadata_bytes = ranges.iter().map(|range| range.text().len()).sum::<usize>();
    let limits = ReadLimits::new().max_metadata_bytes(metadata_bytes);
    assert!(read_parts_with_limits(Parts::new(text.as_bytes()), limits).is_ok());
    assert!(ReadOptions::new().limits(limits).read(&path).is_ok());
    for error in [
        read_parts_with_limits(
            Parts::new(text.as_bytes()),
            limits.max_metadata_bytes(metadata_bytes - 1),
        )
        .unwrap_err(),
        ReadOptions::new()
            .limits(limits.max_metadata_bytes(metadata_bytes - 1))
            .read(&path)
            .unwrap_err(),
    ] {
        assert!(matches!(
            error.reason(),
            ReadErrorReason::LimitExceeded {
                resource: ReadResource::MetadataBytes,
                ..
            }
        ));
    }
    let output = ProcessingPlan::new(vec![ProcessingOperation::ReverseAxis { axis: 0 }])
        .unwrap()
        .apply_processed(disk.processed())
        .unwrap();
    let history = output.provenance().history().unwrap();
    let replayed = history
        .replay_processed(&memory, ProcessingOptions::new())
        .unwrap();
    assert_eq!(replayed.descriptor(), output.descriptor());
    assert_eq!(replayed.data().shape(), &[4]);
    assert_eq!(replayed.data().component_counts(), &[1]);
    assert_eq!(replayed.data().samples(), &[26.0, 24.0, 22.0, 20.0]);
    let comment_change = text.replace("preserved comment", "different comment");
    let changed = read_parts(Parts::new(comment_change.as_bytes())).unwrap();
    assert_eq!(changed.canonical_digests(), memory.canonical_digests());
    assert!(matches!(
        history
            .replay_processed(&changed, ProcessingOptions::new())
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::InputIdentityMismatch)
    ));
    std::fs::write(&path, comment_change).unwrap();
    assert_eq!(
        history
            .replay_processed(disk.processed(), ProcessingOptions::new())
            .unwrap()
            .data()
            .samples(),
        output.data().samples()
    );
}
