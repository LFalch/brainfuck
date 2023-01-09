use std::{io::Error as IoError, result::Result as StdResult};

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug)]
pub enum Error {
    Stopped,
    OutOfBounds,
    NoLoopStarted,
    UnendedLoop,
    CellPointerOverflow,
    IoError(IoError),
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::IoError(e)
    }
}
