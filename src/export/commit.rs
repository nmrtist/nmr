use super::*;

pub(super) fn publish(temporary: NamedTempFile, target: &Path) -> Result<(), ExportError> {
    match temporary.persist_noclobber(target) {
        Ok(_) => Ok(()),
        Err(error) => {
            let source = error.error;
            let temporary_path = error.file.path().to_path_buf();
            let cleanup_failed = error.file.close().is_err();
            if source.kind() == io::ErrorKind::AlreadyExists {
                Err(ExportError::TargetExists)
            } else {
                Err(ExportError::Publish {
                    source,
                    temporary_path,
                    cleanup_failed,
                })
            }
        }
    }
}
