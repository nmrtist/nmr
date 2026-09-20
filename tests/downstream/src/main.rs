//! Independent downstream application and allocation acceptance.

mod allocations;
mod performance;
mod support;
mod workflows;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("performance") => performance::run(),
        _ => {
            allocations::cancelled_nus_preparation_does_not_allocate_index_or_sample_buffers();
            workflows::interactive_workflow();
            workflows::batch();
            workflows::store();
        }
    }
}
