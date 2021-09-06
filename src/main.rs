use crate::{
    manifest::{Country, Manifest},
    problem::{Problem, ProblemList},
    processor::Processor,
};
use anyhow::{anyhow, Context, Result};
use console::Style;
use indicatif::{MultiProgress, ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use rfd::FileDialog;
use std::{
    collections::HashMap,
    fs::{read_dir, remove_file, DirEntry},
    io::{stdin, stdout, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};
use structopt::StructOpt;

mod manifest;
mod problem;
mod processor;

/// Checks downloaded HereV1 maps and (optionally) deletes files that are corrupt so they can be downloaded again by the downloader.
#[derive(Debug, StructOpt)]
pub struct Opt {
    /// The directory where the downloaded maps are stored. Presents a folder-picker if not provided.
    pub dir: Option<PathBuf>,

    /// Delete corrupt files without confirmation.
    #[structopt(short, long)]
    pub force_delete: bool,
}

fn main() -> Result<()> {
    let bold = Style::new().bold();
    let opt: Opt = StructOpt::from_args();
    let path = opt
        .dir
        .or_else(|| {
            println!("Please select the folder that contains the update.xml");
            FileDialog::new().pick_folder()
        })
        .ok_or_else(|| anyhow!("aborted"))?;
    let update_file = path.join("update.xml");

    println!("Using path: {}", bold.apply_to(path.to_string_lossy()));

    let manifest = Manifest::open(&update_file)?;
    let countries = manifest.countries()?;
    let file_count = countries.iter().map(|c| c.file_count()).sum();

    println!(
        "Found maps for region: {} ({} countries in {} files)",
        bold.apply_to(manifest.region_name()),
        bold.apply_to(countries.len()),
        bold.apply_to(file_count)
    );

    let zip_files = find_zip_files(&path)?;

    println!(
        "Found {} relevant files in path",
        bold.apply_to(zip_files.len())
    );

    println!("Performing integrity check...");
    let problems = analyze(&countries, &zip_files, file_count)?;

    println!();
    report_problems(&problems);

    let corrupt = problems.corrupt_files();

    if corrupt.is_empty() {
        println!("No actions needed.");
        return Ok(());
    }

    if !opt.force_delete {
        print!("Do you want to remove the corrupt files? (Y/n) ");
        stdout().flush()?;
        let mut response = String::new();
        stdin().read_line(&mut response)?;
        if !matches!(response.trim(), "" | "y" | "Y") {
            println!("Aborting");
            return Ok(());
        }
    }

    for file in corrupt {
        println!("Removing: {}", file);
        remove_file(path.join(file))?;
    }

    println!("Done.");

    Ok(())
}

fn find_zip_files(path: &Path) -> Result<HashMap<String, DirEntry>> {
    read_dir(path)
        .context("Could not read directory entries")?
        .filter_map(|f| match f {
            Err(e) => Some(Err(e.into())),
            Ok(e) if e.path().extension()? == "zip" => Some(Ok((
                e.path().file_name()?.to_string_lossy().into_owned(),
                e,
            ))),
            _ => None,
        })
        .collect::<Result<HashMap<String, DirEntry>>>()
        .context("Error while reading directory entries")
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
                Processor::create(
                    problems.clone(),
                    bars.add(ProgressBar::new(100))
                        .with_style(bar_style.clone()),
                )
            },
            |processor, expected_file| match zip_files.get(&expected_file.filename) {
                None => problems.lock().unwrap().push(Problem::NotFound {
                    filename: expected_file.filename,
                }),
                Some(actual_file) => processor.process_file(actual_file, expected_file),
            },
        );

    bars_thread.join().unwrap()?;

    Ok(Arc::try_unwrap(problems).unwrap().into_inner().unwrap())
}

fn report_problems(problems: &[Problem]) {
    if problems.is_empty() {
        println!("No problems encountered, you are good to go!");
        return;
    }
    println!("Encountered {} problem(s):", problems.len());
    if let Some(s) = problems.missing_files_msg() {
        println!("- {}", s)
    }
    for p in problems.other_errors() {
        println!("- {}", p);
    }
}
