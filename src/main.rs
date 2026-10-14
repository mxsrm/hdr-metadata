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
//! hdr-metadata --max-frames 100 movie.mkv
//! hdr-metadata --help
//! hdr-metadata --version
//! ```
//!
//! The frame limit defaults to 30 and must be greater than zero. A successful
//! report is written to standard output; failures are written to standard error
//! and produce exit status 1. Invalid CLI arguments produce exit status 2. Missing static HDR
//! metadata is not itself an error.
//!
//! This is a binary crate. The modules below are implementation modules rather
//! than a separately importable library API.

mod encoder;
mod metadata;
mod probe;

use std::{
    error::Error,
    num::NonZeroUsize,
    path::PathBuf,
    process,
};

use clap::{
    Arg,
    Command,
    value_parser,
};

/// Defines the command line using Clap's minimal builder API.
///
/// Clap supplies help, version output, validation, and usage diagnostics. The
/// two frame-limit arguments conflict so a limit cannot be supplied twice.
fn command() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("Extract static HDR10 metadata and print x265 / SVT-AV1 parameters")
        .arg_required_else_help(true)
        .arg(
            Arg::new("video")
                .value_name("VIDEO")
                .help("Input video file")
                .required(true)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("frames")
                .value_name("MAX_FRAMES")
                .help("Decoded-frame limit (default: 30)")
                .value_parser(value_parser!(NonZeroUsize)),
        )
        .arg(
            Arg::new("max-frames")
                .long("max-frames")
                .value_name("N")
                .help("Decoded-frame limit (default: 30)")
                .conflicts_with("frames")
                .value_parser(value_parser!(NonZeroUsize)),
        )
}

/// Parses arguments with Clap, then probes the video and prints its metadata.
///
/// Help and version requests exit before FFmpeg initialization. Clap handles
/// invalid arguments with exit status 2; probing errors are returned to `main`.
///
/// # Errors
///
/// Returns an error if [`probe::probe`] fails.
///
/// # Panics
///
/// Panics if the command definition fails to provide its required video path,
/// or if printing the report fails.
fn run() -> Result<(), Box<dyn Error>> {
    let matches = command().get_matches();
    let path = matches.get_one::<PathBuf>("video").expect("VIDEO is required by Clap");
    let max_frames = matches
        .get_one::<NonZeroUsize>("max-frames")
        .or_else(|| matches.get_one::<NonZeroUsize>("frames"))
        .map(|count| count.get())
        .unwrap_or(30);
    let hdr = probe::probe(path, max_frames)?;
    encoder::print_report(path, &hdr);
    Ok(())
}

/// Reports a failure and exits with a nonzero status.
fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;

    use super::*;

    #[test]
    fn command_definition_is_consistent() {
        command().debug_assert();
    }

    #[test]
    fn clap_handles_help_and_version_without_media() {
        for flag in ["-h", "--help"] {
            let error = command().try_get_matches_from(["hdr-metadata", flag]).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::DisplayHelp);
            assert_eq!(error.exit_code(), 0);
        }
        for flag in ["-V", "--version"] {
            let error = command().try_get_matches_from(["hdr-metadata", flag]).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::DisplayVersion);
            assert_eq!(error.exit_code(), 0);
        }
    }

    #[test]
    fn accepts_both_frame_limit_forms() {
        for args in [
            vec!["hdr-metadata", "movie.mkv", "100"],
            vec!["hdr-metadata", "--max-frames", "100", "movie.mkv"],
        ] {
            let matches = command().try_get_matches_from(args).expect("valid arguments");
            let count = matches
                .get_one::<NonZeroUsize>("max-frames")
                .or_else(|| matches.get_one::<NonZeroUsize>("frames"));
            assert_eq!(count.map(|n| n.get()), Some(100));
        }
    }

    #[test]
    fn rejects_zero_and_conflicting_limits() {
        for args in [
            vec!["hdr-metadata", "movie.mkv", "0"],
            vec!["hdr-metadata", "--max-frames", "0", "movie.mkv"],
            vec!["hdr-metadata", "movie.mkv", "100", "--max-frames", "200"],
        ] {
            assert!(command().try_get_matches_from(args).is_err());
        }
    }
}
