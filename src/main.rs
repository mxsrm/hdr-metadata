//! Extracts static HDR10 metadata and prints encoder parameters.
//!
//! This binary links to FFmpeg directly. It searches the selected video stream
//! for mastering-display and content-light metadata, then prints parameters for
//! x265 and SVT-AV1. Dynamic HDR10+ and Dolby Vision metadata are not extracted.
//!
//! # Usage
//!
//! ```text
//! hdr-metadata <video> [max_frames]
//! hdr-metadata movie.mkv 100
//! ```
//!
//! The frame limit defaults to 30 and must be greater than zero. A successful
//! report is written to standard output; failures are written to standard error
//! and produce exit status 1. Missing static HDR metadata is not itself an error.
//!
//! This is a binary crate. The modules below are implementation modules rather
//! than a separately importable library API.

mod encoder;
mod metadata;
mod probe;

use std::{
    env,
    error::Error,
    path::PathBuf,
    process,
};

/// Parses the command-line arguments and prints a metadata report.
///
/// Uses native operating-system strings for the input path so that paths need
/// not be valid UTF-8. Only the optional frame limit is converted to text.
///
/// # Errors
///
/// Returns an error for a missing path, an invalid or zero frame limit, extra
/// arguments, or a failure from [`probe::probe`].
fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let path = PathBuf::from(args.next().ok_or("Usage: hdr-metadata <video> [max_frames]")?);
    let max_frames = args
        .next()
        .map(|s| s.to_string_lossy().parse::<usize>())
        .transpose()?
        .unwrap_or(30);
    if max_frames == 0 || args.next().is_some() {
        return Err("Usage: hdr-metadata <video> [max_frames > 0]".into());
    }
    let hdr = probe::probe(&path, max_frames)?;
    encoder::print_report(&path, &hdr);
    Ok(())
}

/// Reports a failure and exits with a nonzero status.
fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(1);
    }
}
