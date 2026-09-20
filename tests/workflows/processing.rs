#[path = "../support/workflows.rs"]
mod workflows;
#[test]
fn public_end_to_end_workflows_cover_dense_mixed_pseudo_nus_and_offline_derivations() {
    assert_eq!(workflows::run().unwrap().len(), 26);
}
