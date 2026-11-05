use std::fmt;

/// Error type shared by every format reader.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// A read ran past the end of the data.
    Eof {
        offset: usize,
        wanted: usize,
        len: usize,
    },
    /// The data did not match the format.
    Format(String),
    /// A named resource was not found.
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn format(msg: impl Into<String>) -> Self {
        Error::Format(msg.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::Eof {
                offset,
                wanted,
                len,
            } => {
                write!(f, "read of {wanted} bytes at {offset} past end ({len})")
            }
            Error::Format(m) => write!(f, "bad format: {m}"),
            Error::NotFound(n) => write!(f, "not found: {n}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
