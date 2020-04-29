#[macro_use]
extern crate error_chain;

use console::Style;
use indicatif::{MultiProgress, ParallelProgressIterator, ProgressBar, ProgressStyle};
use quick_xml::de::from_reader;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::{read_dir, File};
use std::io::{prelude::*, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use structopt::StructOpt;

mod manifest;
use manifest::*;

mod errors {
    error_chain! {
        foreign_links {
            Io(std::io::Error);
            Xml(quick_xml::DeError);
        }
    }
}
use errors::*;

#[derive(Debug, StructOpt)]
/// Checks downloaded HereV1 maps and (optionally) deletes files that are corrupt so they can be downloaded again by the downloader.
pub struct Opt {
    /// The directory where the downloaded maps are stored.
    pub dir: PathBuf,

    /// Delete corrupt files without confirmation.
    #[structopt(short, long)]
    pub force_delete: bool,
}

quick_main!(run);

fn run() -> Result<()> {
    let bold = Style::new().bold();
    let opt = Opt::from_args();
    let path = opt.dir;
    let update_file = {
        let mut buf = path.clone();
        buf.push("update.xml");
        buf
    };

    println!("Using path: {}", bold.apply_to(path.to_string_lossy()));

    let manifest: Manifest = with_message(
        "reading update.xml",
        || format!("Could not read: {:?}", update_file),
        || {
            let file = File::open(&update_file)?;
            let parsed = from_reader(BufReader::new(file))?;
            Ok(parsed)
        },
    )?;

    let countries = manifest.countries()?;
    let file_count = countries.iter().map(|c| c.file_count()).sum();

    println!(
        "Found maps for region: {} ({} countries in {} files)",
        bold.apply_to(manifest.region_name()),
        bold.apply_to(countries.len()),
        bold.apply_to(file_count)
    );

    let zip_files: HashMap<_, _> = with_message(
        "reading dir entries",
        || format!("Could not read dir: {:?}", path),
        || {
            Ok(read_dir(&path)?
                .filter_map(|f| match f {
                    Err(e) => Some(Err(e.into())),
                    Ok(e) if e.path().extension()? == "zip" => Some(Ok((
                        e.path().file_name()?.to_string_lossy().into_owned(),
                        e,
                    ))),
                    _ => None,
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .collect())
        },
    )?;

    println!(
        "Found {} relevant files in path",
        bold.apply_to(zip_files.len())
    );

    println!("Performing integrity check...");

    let bars = Arc::new(MultiProgress::new());
    let main_bar = bars.add(ProgressBar::new(file_count)).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40} file {pos:.bold} of {len:.bold} ({eta})"),
    );
    let bars_thread = {
        let bars = bars.clone();
        thread::spawn(move || {
            bars.join_and_clear().unwrap();
        })
    };

    let problems = Arc::new(Mutex::new(vec![]));
    let sty =
        ProgressStyle::default_bar().template("[{elapsed_precise}] {bar:40.cyan/blue} ({total_bytes:>8}) {wide_msg}");
    countries
        .par_iter()
        .flat_map(|c| c.files())
        .progress_with(main_bar)
        .for_each_init(
            || {
                (
                    problems.clone(),
                    bars.add(ProgressBar::new(100)).with_style(sty.clone()),
                )
            },
            |(problems, bar), file| match zip_files.get(&file.filename) {
                None => problems.lock().unwrap().push(Problem::NotFound {
                    filename: file.filename,
                }),
                Some(zip_file) => {
                    let zip_size = zip_file.metadata().unwrap().len();
                    let size = file.packedsize();
                    if zip_size != size {
                        problems.lock().unwrap().push(Problem::WrongSize {
                            filename: file.filename,
                            expected: size,
                            got: zip_size,
                        });
                        return;
                    }
                    bar.set_message(&file.filename);
                    bar.set_length(size);
                    let expected = file.md5().to_string();
                    match get_md5(bar, zip_file.path()) {
                        Err(e) => problems.lock().unwrap().push(Problem::Error(e.into())),
                        Ok(got) if got == expected => (),
                        Ok(got) => problems.lock().unwrap().push(Problem::WrongSignature {
                            filename: file.filename,
                            got,
                            expected,
                        }),
                    }
                }
            },
        );

    bars_thread.join().unwrap();
    println!("Problems: {:#?}", problems.lock().unwrap());

    println!("ZIP files: {}", zip_files.len());
    Ok(())
}

fn with_message<R, EK: Into<ErrorKind>>(
    msg: &str,
    error: impl FnOnce() -> EK,
    f: impl FnOnce() -> Result<R>,
) -> Result<R> {
    let pb = ProgressBar::new_spinner();
    pb.set_message(&format!("{}", msg));
    pb.enable_steady_tick(100);
    f().chain_err(error)
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

#[derive(Debug)]
enum Problem {
    NotFound {
        filename: String,
    },
    WrongSize {
        filename: String,
        expected: u64,
        got: u64,
    },
    WrongSignature {
        filename: String,
        expected: String,
        got: String,
    },
    Error(errors::Error),
}
