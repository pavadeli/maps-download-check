use crate::{manifest::ZipFile, problem::Problem};
use anyhow::Result;
use indicatif::ProgressBar;
use std::{
    fs::{DirEntry, File},
    io::copy,
    path::Path,
    sync::{Arc, Mutex},
};

pub struct Processor {
    problems: Arc<Mutex<Vec<Problem>>>,
    bar: ProgressBar,
}

impl Processor {
    pub fn create(problems: Arc<Mutex<Vec<Problem>>>, bar: ProgressBar) -> Self {
        Self { problems, bar }
    }

    pub fn process_file(&mut self, actual_file: &DirEntry, expected_file: ZipFile) {
        if let Err(e) = self.try_process_file(actual_file, expected_file) {
            self.problems
                .lock()
                .unwrap()
                .push(e.downcast().unwrap_or_else(|e| e.into()))
        }
    }

    fn try_process_file(&mut self, actual_file: &DirEntry, expected_file: ZipFile) -> Result<()> {
        let size = expected_file.packedsize;
        let zip_size = actual_file.metadata()?.len();
        if zip_size != size {
            // Move the bar to the right to indicate progress, even if we didn't actually read any bytes.
            self.bar.inc(size);
            return Err(Problem::WrongSize {
                filename: expected_file.filename,
                expected: size,
                got: zip_size,
            }
            .into());
        }
        let expected = expected_file.md5;
        let got = self.get_md5(&actual_file.path())?;
        if got != expected {
            let expected = expected.to_string();
            return Err(Problem::WrongSignature {
                filename: expected_file.filename,
                got,
                expected,
            }
            .into());
        }
        Ok(())
    }

    fn get_md5(&mut self, path: &Path) -> Result<String> {
        let file = File::open(path)?;
        let mut context = md5::Context::new();
        copy(&mut self.bar.wrap_read(file), &mut context)?;
        Ok(format!("{:x}", context.compute()))
    }
}
