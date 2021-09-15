use crate::{manifest::ZipFile, problem::Problem};
use anyhow::Result;
use std::{
    fs::{DirEntry, File},
    io::Read,
    path::Path,
    sync::{Arc, Mutex},
};

const BUF_SIZE: usize = 8 * 1024;

pub struct Processor {
    problems: Arc<Mutex<Vec<Problem>>>,
    buf: [u8; BUF_SIZE],
}

impl Processor {
    pub fn create(problems: Arc<Mutex<Vec<Problem>>>) -> Self {
        Self {
            problems,
            buf: [0; BUF_SIZE],
        }
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
        let size = expected_file.packedsize();
        let zip_size = actual_file.metadata()?.len();
        if zip_size != size {
            return Err(Problem::WrongSize {
                filename: expected_file.filename,
                expected: size,
                got: zip_size,
            }
            .into());
        }
        let expected = expected_file.md5();
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
        let mut file = File::open(path)?;
        let mut context = md5::Context::new();
        loop {
            let n = file.read(&mut self.buf)?;
            if n == 0 {
                break;
            }
            context.consume(&self.buf[..n]);
        }
        Ok(format!("{:x}", context.compute()))
    }
}
