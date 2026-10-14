use std::{
    error::Error,
    mem::size_of,
    ptr,
};

use ffmpeg_next::{
    ffi,
    frame,
    util::frame::side_data::Type,
};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct MasteringDisplay {
    pub red: Point,
    pub green: Point,
    pub blue: Point,
    pub white: Point,
    pub min_luminance: f64,
    pub max_luminance: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct ContentLight {
    pub max_cll: u32,
    pub max_fall: u32,
}

#[derive(Debug, Default)]
pub struct HdrMetadata {
    pub color_space: String,
    pub color_primaries: String,
    pub color_transfer: String,
    pub mastering: Option<MasteringDisplay>,
    pub cll: Option<ContentLight>,
    pub frames_scanned: usize,
}

use std::os::raw::{
    c_int,
    c_uint,
};

// Keep your existing `ffi` import.

#[repr(C)]
#[derive(Clone, Copy)]
struct AVMasteringDisplayMetadata {
    display_primaries: [[ffi::AVRational; 2]; 3],
    white_point: [ffi::AVRational; 2],
    min_luminance: ffi::AVRational,
    max_luminance: ffi::AVRational,
    has_primaries: c_int,
    has_luminance: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case)]
struct AVContentLightMetadata {
    MaxCLL: c_uint,
    MaxFALL: c_uint,
}

fn rational(r: ffi::AVRational) -> Result<f64> {
    if r.den == 0 {
        return Err("Invalid HDR rational: zero denominator".into());
    }
    Ok(f64::from(r.num) / f64::from(r.den))
}

fn point(p: [ffi::AVRational; 2]) -> Result<Point> {
    Ok(Point {
        x: rational(p[0])?,
        y: rational(p[1])?,
    })
}

// FFmpeg owns the side-data bytes. Check the payload size, then copy them with
// read_unaligned so this remains sound even if a buffer isn't aligned for T.
fn read_payload<T>(bytes: &[u8]) -> Result<T> {
    if bytes.len() < size_of::<T>() {
        return Err("Truncated FFmpeg HDR side data".into());
    }
    // SAFETY: size checked; read_unaligned copies the bytes without referencing
    // them as an aligned T. The two FFmpeg C structures used here contain only
    // integer fields and AVRational values (all bit patterns are valid).
    Ok(unsafe { ptr::read_unaligned(bytes.as_ptr().cast::<T>()) })
}

impl HdrMetadata {
    pub fn update_from_frame(&mut self, frame: &frame::Video) -> Result<()> {
        self.frames_scanned += 1;
        if self.color_space == "unknown" {
            self.color_space = format!("{:?}", frame.color_space());
        }
        if self.color_primaries == "unknown" {
            self.color_primaries = format!("{:?}", frame.color_primaries());
        }
        if self.color_transfer == "unknown" {
            self.color_transfer = format!("{:?}", frame.color_transfer_characteristic());
        }
        if self.mastering.is_none()
            && let Some(side) = frame.side_data(Type::MasteringDisplayMetadata)
        {
            let raw: AVMasteringDisplayMetadata = read_payload(side.data())?;
            // FFmpeg orders primaries red, green, blue.
            if raw.has_primaries != 0 && raw.has_luminance != 0 {
                self.mastering = Some(MasteringDisplay {
                    red: point(raw.display_primaries[0])?,
                    green: point(raw.display_primaries[1])?,
                    blue: point(raw.display_primaries[2])?,
                    white: point(raw.white_point)?,
                    min_luminance: rational(raw.min_luminance)?,
                    max_luminance: rational(raw.max_luminance)?,
                });
            }
        }
        if self.cll.is_none()
            && let Some(side) = frame.side_data(Type::ContentLightLevel)
        {
            let raw: AVContentLightMetadata = read_payload(side.data())?;
            self.cll = Some(ContentLight {
                max_cll: raw.MaxCLL,
                max_fall: raw.MaxFALL,
            });
        }
        Ok(())
    }

    pub fn complete(&self) -> bool {
        self.mastering.is_some() && self.cll.is_some()
    }
}

impl Default for Point {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl HdrMetadata {
    pub fn empty() -> Self {
        Self {
            color_space: "unknown".into(),
            color_primaries: "unknown".into(),
            color_transfer: "unknown".into(),
            ..Self::default()
        }
    }
}
