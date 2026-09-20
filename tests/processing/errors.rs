use nmr::processing::{
    ProcessingErrorCode, ProcessingOperation as Op, ProcessingOptions, ProcessingPlan,
};

use crate::datasets::*;

#[test]
fn repeated_operations_report_the_exact_caller_step() {
    let baseline = Op::BaselineCorrection {
        axis: 0,
        profile: nmr::processing::PositivePeaksV1.into(),
    };
    let plan = ProcessingPlan::new(vec![
        baseline.clone(),
        Op::ReverseAxis { axis: 0 },
        baseline,
    ])
    .unwrap();
    let error = plan
        .preflight(&scalar(65), ProcessingOptions::default())
        .unwrap_err();
    assert_eq!(error.step_index(), Some(2));
    assert_eq!(error.code(), ProcessingErrorCode::InvalidState);
    assert!(matches!(
        error,
        nmr::processing::ProcessingError::Step {
            phase: nmr::processing::ProcessingPhase::Preflight,
            target: Some(nmr::processing::OperationTarget::Axis(0)),
            ..
        }
    ));
}
