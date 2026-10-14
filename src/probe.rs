use crate::metadata::{HdrMetadata, Result};
use ffmpeg_next::{codec, format, frame, media};
use std::path::Path;

pub fn probe(path: &Path, max_frames: usize) -> Result<HdrMetadata> {
    ffmpeg_next::init()?;
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
    // Stream properties are fallback metadata (may be unspecified).
    hdr.color_space = format!("{:?}", decoder.color_space());
    hdr.color_primaries = format!("{:?}", decoder.color_primaries());
    hdr.color_transfer = format!("{:?}", decoder.color_transfer_characteristic());

    let mut decoded = frame::Video::empty();
    for (stream, packet) in input.packets() {
        if stream.index() != stream_index {
            continue;
        }
        decoder.send_packet(&packet)?;
        while decoder.receive_frame(&mut decoded).is_ok() {
            hdr.update_from_frame(&decoded)?;
            if hdr.complete() || hdr.frames_scanned >= max_frames {
                return Ok(hdr);
            }
        }
    }

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
