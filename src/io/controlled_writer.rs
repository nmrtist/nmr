//! Bounded writes with cooperative cancellation and actual-byte accounting.

use crate::execution::{ExecutionContext, ExecutionError};

pub(crate) struct ControlledWriter<'a, 'ctx, W> {
    pub writer: &'a mut W,
    pub control: &'a mut ExecutionContext<'ctx>,
    pub count_io: bool,
}
impl<W: std::io::Write> std::io::Write for ControlledWriter<'_, '_, W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.control
            .check_cancelled()
            .map_err(std::io::Error::other)?;
        let count = self.writer.write(&bytes[..bytes.len().min(32768)])?;
        if self.count_io {
            self.control.io(count)
        } else {
            self.control.advance(count as u128)
        }
        .map_err(std::io::Error::other)?;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.control
            .check_cancelled()
            .map_err(std::io::Error::other)?;
        self.writer.flush()
    }
}

pub(crate) fn io_control_error(error: &std::io::Error) -> Option<ExecutionError> {
    error
        .get_ref()
        .and_then(|e| e.downcast_ref::<ExecutionError>())
        .copied()
}
