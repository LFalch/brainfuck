use std::io::Error as IoError;
use std::result::Result as StdResult;

use crate::chars::CharsError;

#[must_use]
pub type Result<T> = StdResult<T, Error>;

#[derive(Debug)]
pub enum Error {
    Exit,
    OutOfBounds,
    NoBlockStarted,
    IoError(IoError),
    CharsError(CharsError),
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::IoError(e)
    }
}
