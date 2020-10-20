pub type Result<T> = std::result::Result<T, failure::Error>;

#[derive(Debug, Fail)]
pub enum Problem {
    #[fail(display = "File {} was not found", filename)]
    NotFound { filename: String },
    #[fail(
        display = "File {} has size: {}, expected: {}",
        filename, got, expected
    )]
    WrongSize {
        filename: String,
        expected: u64,
        got: u64,
    },
    #[fail(
        display = "File {} has signature: {:?}, expected: {:?}",
        filename, got, expected
    )]
    WrongSignature {
        filename: String,
        expected: String,
        got: String,
    },
    #[fail(display = "Error: {}", _0)]
    Error(failure::Error),
}
