#[macro_use]
extern crate failure;

use console::Style;
use failure::{Error, Fail, ResultExt};
use indicatif::{MultiProgress, ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::{read_dir, DirEntry, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use structopt::StructOpt;

mod manifest;
use manifest::*;

/// Checks downloaded HereV1 maps and (optionally) deletes files that are corrupt so they can be downloaded again by the downloader.
#[derive(Debug, StructOpt)]
pub struct Opt {
    /// The directory where the downloaded maps are stored.
    pub dir: PathBuf,

    /// Delete corrupt files without confirmation.
    #[structopt(short, long)]
    pub force_delete: bool,
}

type Result<T> = std::result::Result<T, Error>;

fn main() -> Result<()> {
    let bold = Style::new().bold();
    let opt: Opt = StructOpt::from_args();
    let path = opt.dir;
    let update_file = {
        let mut buf = path.clone();
        buf.push("update.xml");
        buf
    };

    println!("Using path: {}", bold.apply_to(path.to_string_lossy()));

    let manifest = Manifest::open(update_file)?;
    let countries = manifest.countries()?;
    let file_count = countries.iter().map(|c| c.file_count()).sum();

    println!(
        "Found maps for region: {} ({} countries in {} files)",
        bold.apply_to(manifest.region_name()),
        bold.apply_to(countries.len()),
        bold.apply_to(file_count)
    );

    let zip_files = find_zip_files(path)?;

    println!(
        "Found {} relevant files in path",
        bold.apply_to(zip_files.len())
    );

    println!("Performing integrity check...");
    let problems = analyze(&countries, &zip_files, file_count)?;

    report_problems(&problems);

    // TODO: implement confirmation and deletion logic

    Ok(())
}

fn get_md5(bar: &ProgressBar, path: impl AsRef<Path>) -> Result<String> {
    let mut file = File::open(path)?;
    let mut context = md5::Context::new();
    let mut buffer = [0; 8 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bar.inc(n as u64);
        context.consume(&buffer[..n]);
    }
    Ok(format!("{:x}", context.compute()))
}

fn find_zip_files(path: impl AsRef<Path>) -> Result<HashMap<String, DirEntry>> {
    Ok(read_dir(path)
        .context("Could not read directory entries")?
        .filter_map(|f| match f {
            Err(e) => Some(Err(e.into())),
            Ok(e) if e.path().extension()? == "zip" => Some(Ok((
                e.path().file_name()?.to_string_lossy().into_owned(),
                e,
            ))),
            _ => None,
        })
        .collect::<Result<Vec<_>>>()
        .context("Error while reading directory entries")?
        .into_iter()
        .collect())
}

fn analyze(
    countries: &[&Country],
    zip_files: &HashMap<String, DirEntry>,
    file_count: u64,
) -> Result<Vec<Problem>> {
    let bars = Arc::new(MultiProgress::new());
    let main_bar = bars.add(ProgressBar::new(file_count)).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40} file {pos:.bold} of {len:.bold} ({eta})"),
    );
    let bars_thread = {
        let bars = bars.clone();
        thread::spawn(move || bars.join_and_clear())
    };
    let problems: Arc<Mutex<Vec<Problem>>> = Arc::default();
    let bar_style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} ({total_bytes:>8}) {wide_msg}");

    countries
        .par_iter()
        .flat_map(|c| c.files())
        .progress_with(main_bar)
        .for_each_init(
            || {
                (
                    problems.clone(),
                    bars.add(ProgressBar::new(100))
                        .with_style(bar_style.clone()),
                )
            },
            |(problems, bar), expected_file| match zip_files.get(&expected_file.filename) {
                None => problems.lock().unwrap().push(Problem::NotFound {
                    filename: expected_file.filename,
                }),
                Some(actual_file) => {
                    let size = expected_file.packedsize();
                    bar.set_message(&expected_file.filename);
                    bar.set_length(size);
                    let result = (|| -> Result<()> {
                        let zip_size = actual_file.metadata()?.len();
                        if zip_size != size {
                            Err(Problem::WrongSize {
                                filename: expected_file.filename.clone(),
                                expected: size,
                                got: zip_size,
                            })?;
                        }
                        let expected = expected_file.md5();
                        let got = get_md5(bar, actual_file.path())?;
                        if got != expected {
                            Err(Problem::WrongSignature {
                                filename: expected_file.filename.clone(),
                                got,
                                expected: expected.to_string(),
                            })?;
                        }
                        Ok(())
                    })();
                    if let Err(e) = result {
                        match e.downcast() {
                            Ok(p) => problems.lock().unwrap().push(p),
                            Err(e) => problems.lock().unwrap().push(Problem::Error(e)),
                        }
                    }
                }
            },
        );

    bars_thread.join().unwrap()?;

    Ok(Arc::try_unwrap(problems).unwrap().into_inner().unwrap())
}

fn report_problems(problems: &[Problem]) {
    println!();
    let (missing, other): (Vec<&Problem>, Vec<&Problem>) = problems
        .iter()
        .partition(|&p| matches!(p, Problem::NotFound{..}));
    let missing: Vec<&str> = missing
        .into_iter()
        .map(|p| match p {
            Problem::NotFound { filename } => &filename[..],
            _ => panic!("should not occur"),
        })
        .collect();
    match missing.len() + other.len() {
        0 => println!("No problems encountered, you are good to go!"),
        n => {
            println!("Encountered {} problem(s):", n);
            println!("- {} missing files: {}", missing.len(), missing.join(", "));
            for problem in &*other {
                println!("- {}", problem);
            }
        }
    }
}

#[derive(Debug, Fail)]
enum Problem {
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
    Error(Error),
}
