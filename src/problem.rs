use thiserror::Error;

#[derive(Debug, Error)]
pub enum Problem {
    #[error("File {filename} was not found")]
    NotFound { filename: String },
    #[error("File {filename} has size: {got}, expected: {expected}")]
    WrongSize {
        filename: String,
        expected: u64,
        got: u64,
    },
    #[error("File {filename} has signature: {got:?}, expected: {expected:?}")]
    WrongSignature {
        filename: String,
        expected: String,
        got: String,
    },
    #[error(transparent)]
    Error(#[from] anyhow::Error),
}

pub trait ProblemList {
    fn missing_files_msg(&self) -> Option<String>;
    fn other_errors(&self) -> Vec<&Problem>;
    fn corrupt_files(&self) -> Vec<&str>;
}

impl ProblemList for [Problem] {
    fn missing_files_msg(&self) -> Option<String> {
        let filenames: Vec<_> = self
            .iter()
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
        self.iter()
            .filter(|p| !matches!(p, Problem::NotFound { .. }))
            .collect()
    }

    fn corrupt_files(&self) -> Vec<&str> {
        self.iter()
            .filter_map(|p| match p {
                Problem::WrongSignature { filename, .. } | Problem::WrongSize { filename, .. } => {
                    Some(&filename[..])
                }
                _ => None,
            })
            .collect()
    }
}
