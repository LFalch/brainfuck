use std::io::Error as IoError;
use std::result::Result as StdResult;

#[must_use]
pub type Result<T> = StdResult<T, Error>;

#[derive(Debug)]
pub enum Error {
    Stopped,
    OutOfBounds,
    NoLoopStarted,
    UnendedLoop,
    IoError(IoError),
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::IoError(e)
    }
}
