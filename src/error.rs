
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MdbError {
    #[error("out of bounds")]
    OutOfBounds(String),
    #[error("no data type available")]
    NoDataTypeAvailable(String),
    #[error("invalid mdb")]
    InvalidMdb(String),
    #[error("invalid value")]
    InvalidValue(String),
    #[error("out of range")]
    OutOfRange(String),
}

impl From<std::num::ParseIntError> for MdbError {
    fn from(e: std::num::ParseIntError) -> MdbError {
        return MdbError::InvalidValue(format!("{}", e));
    }
}