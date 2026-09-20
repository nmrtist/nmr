use nmr::raw::{ChemicalShiftReference, ResolutionAuthority, TrustClass};

#[test]
fn chemical_shift_reference_has_a_fixed_frequency_sign_and_caller_evidence() {
    let reference = ChemicalShiftReference::user_constructed(4.7, 400.0).unwrap();
    assert_eq!(reference.ppm(400.0).unwrap(), 5.7);
    assert_eq!(reference.ppm(-400.0).unwrap(), 3.7);
    assert_eq!(reference.evidence().trust_class(), TrustClass::UserProvided);
    assert_eq!(
        reference.evidence().authority(),
        &ResolutionAuthority::UserConstructed
    );
}

#[test]
fn chemical_shift_reference_rejects_non_finite_or_non_positive_values() {
    assert!(ChemicalShiftReference::user_constructed(f64::NAN, 400.0).is_err());
    assert!(ChemicalShiftReference::user_constructed(0.0, 0.0).is_err());
    assert!(ChemicalShiftReference::user_constructed(0.0, -400.0).is_err());
    assert!(ChemicalShiftReference::user_constructed(0.0, f64::INFINITY).is_err());
}
