mod encoder;
mod metadata;
mod probe;

use std::{
    env,
    error::Error,
    path::PathBuf,
    process,
};

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

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(1);
    }
}
