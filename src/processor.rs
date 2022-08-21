use crate::{manifest::ZipFile, problem::Problem};
use anyhow::Result;
use indicatif::ProgressBar;
use std::{
    fs::{DirEntry, File},
    io::copy,
    path::Path,
};

pub fn process_file(
    bar: &mut ProgressBar,
    actual_file: &DirEntry,
    expected_file: ZipFile,
) -> Option<Problem> {
    try_process_file(bar, actual_file, expected_file)
        .err()
        .map(|err| err.downcast().unwrap_or_else(Problem::Error))
}

fn try_process_file(
    bar: &mut ProgressBar,
    actual_file: &DirEntry,
    expected_file: ZipFile,
) -> Result<()> {
    let size = expected_file.packedsize;
    let zip_size = actual_file.metadata()?.len();
    if zip_size != size {
        // Move the bar to the right to indicate progress, even if we didn't actually read any bytes.
        bar.inc(size);
        return Err(Problem::WrongSize {
            filename: expected_file.filename,
            expected: size,
            got: zip_size,
        }
        .into());
    }
    let expected = expected_file.md5;
    let got = get_md5(bar, &actual_file.path())?;
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

fn get_md5(bar: &mut ProgressBar, path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mut context = md5::Context::new();
    copy(&mut bar.wrap_read(file), &mut context)?;
    Ok(format!("{:x}", context.compute()))
}
