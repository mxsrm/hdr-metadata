//! Demuxes and decodes video in process to find static HDR10 metadata.
//!
//! FFmpeg selects the best video stream. Only that stream's packets are sent to
//! its decoder; frames are scanned until both records are found or the limit is
//! reached. Decoder-owned frames are reused after their metadata has been copied.

use std::path::Path;

use ffmpeg_next::{codec, format, frame, media};

use crate::metadata::{HdrMetadata, Result};

/// Opens a video and scans decoded frames for static HDR10 metadata.
///
/// `max_frames` limits decoded frames, not packets. Scanning can stop earlier
/// when [`HdrMetadata::complete`] is true. The CLI requires a positive limit;
/// a direct call with zero still processes the first available decoded frame.
///
/// Initial color names come from the decoder's stream properties. FFmpeg logging
/// is set process-wide to errors only; the previous log level is not restored.
/// Missing HDR records are returned as `None`, not as a probing error.
///
/// # Errors
///
/// Returns an error when FFmpeg initialization, opening the input, finding a
/// video stream, opening its decoder, sending packets, signaling end of input,
/// or parsing recognized HDR side data fails. Also returns an error if no frames
/// were decoded. Receive-frame errors end the current drain loop and are not
/// propagated by this implementation.
///
/// # Example
///
/// The CLI uses a 100-frame limit for this invocation:
///
/// ```text
/// hdr-metadata movie.mkv 100
/// ```
pub fn probe(path: &Path, max_frames: usize) -> Result<HdrMetadata> {
    ffmpeg_next::init()?;
    // This affects all FFmpeg users in the process, not only this decoder.
    ffmpeg_next::util::log::set_level(ffmpeg_next::util::log::Level::Error);
    let mut input = format::input(path)?;
    let stream = input
        .streams()
        .best(media::Type::Video)
        .ok_or("No video stream found")?;
    let stream_index = stream.index();
    let context = codec::context::Context::from_parameters(stream.parameters())?;
    let mut decoder = context.decoder().video()?;

    let mut hdr = HdrMetadata::empty();
    // Preserve the decoder's stream-level color names. Even an "Unspecified"
    // debug name replaces the updater's literal "unknown" sentinel.
    hdr.color_space = format!("{:?}", decoder.color_space());
    hdr.color_primaries = format!("{:?}", decoder.color_primaries());
    hdr.color_transfer = format!("{:?}", decoder.color_transfer_characteristic());

    let mut decoded = frame::Video::empty();
    for (stream, packet) in input.packets() {
        if stream.index() != stream_index {
            continue;
        }
        decoder.send_packet(&packet)?;
        // A packet may yield several frames or require more input before any
        // frame is available. Drain available frames before sending another one.
        while decoder.receive_frame(&mut decoded).is_ok() {
            hdr.update_from_frame(&decoded)?;
            if hdr.complete() || hdr.frames_scanned >= max_frames {
                return Ok(hdr);
            }
        }
    }

    // Flush delayed frames (for example, frames held for display reordering)
    // after the demuxer stops yielding packets.
    decoder.send_eof()?;
    while decoder.receive_frame(&mut decoded).is_ok() {
        hdr.update_from_frame(&decoded)?;
        if hdr.complete() || hdr.frames_scanned >= max_frames {
            break;
        }
    }
    if hdr.frames_scanned == 0 {
        return Err("No decoded video frames found".into());
    }
    Ok(hdr)
}
