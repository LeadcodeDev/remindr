use remindr_core::ports::StoreError;

pub(crate) fn read_error(error: rusqlite::Error) -> StoreError {
    StoreError::Read(Box::new(error))
}

pub(crate) fn write_error(error: rusqlite::Error) -> StoreError {
    StoreError::Write(Box::new(error))
}
