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

pub trait ProblemList {
    fn missing_files_msg(&self) -> Option<String>;
    fn other_errors(&self) -> Vec<&Problem>;
    fn corrupt_files(&self) -> Vec<&str>;
}

impl ProblemList for [Problem] {
    fn missing_files_msg(&self) -> Option<String> {
        let filenames: Vec<_> = self
            .into_iter()
            .filter_map(|p| match p {
                Problem::NotFound { filename } => Some(&filename[..]),
                _ => None,
            })
            .collect();
        if filenames.is_empty() {
            return None;
        }
        let mut s = format!(
            "{} missing files: {}",
            filenames.len(),
            filenames.join(", ")
        );
        if s.len() > 80 {
            s.truncate(77);
            s.push_str("...");
        }
        Some(s)
    }

    fn other_errors(&self) -> Vec<&Problem> {
        self.into_iter()
            .filter(|p| !matches!(p, Problem::NotFound{..}))
            .collect()
    }

    fn corrupt_files(&self) -> Vec<&str> {
        self.into_iter()
            .filter_map(|p| match p {
                Problem::WrongSignature { filename, .. } | Problem::WrongSize { filename, .. } => {
                    Some(&filename[..])
                }
                _ => None,
            })
            .collect()
    }
}
