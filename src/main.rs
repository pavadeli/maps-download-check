use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
/// Checks downloaded HereV1 maps and (optionally) deletes files that are corrupt so they can be downloaded again by the downloader.
struct Opt {
    /// The directory where the downloaded maps are stored.
    dir: PathBuf,

    /// Do not delete corrupt files
    #[structopt(short, long)]
    keep_corrupt_files: bool,
}

#[paw::main]
fn main(opt: Opt) {
    println!("{:?}", opt);
}
