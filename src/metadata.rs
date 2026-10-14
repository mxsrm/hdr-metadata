//! Stores static HDR10 metadata and reads FFmpeg frame side data.
//!
//! Chromaticity uses dimensionless CIE 1931 x/y coordinates; luminance uses
//! cd/m². [`HdrMetadata::update_from_frame`] keeps the first complete mastering
//! display and the first content-light record found during a scan.
//!
//! The private C structures mirror `libavutil/mastering_display_metadata.h`.
//! Their layout must match the linked FFmpeg installation. FFmpeg does not
//! guarantee their sizes as part of its public ABI, so these declarations need
//! review when changing the supported FFmpeg version.

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

/// A metadata or probing result with a dynamically typed error.
///
/// Allows FFmpeg failures and payload-validation errors to propagate through
/// the same result type.
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// A dimensionless CIE 1931 chromaticity coordinate pair.
///
/// Coordinates are stored without range validation. [`Default::default`]
/// produces `(0.0, 0.0)`, which does not represent missing metadata.
#[derive(Debug, Clone, Copy)]
pub struct Point {
    /// The x chromaticity coordinate.
    pub x: f64,
    /// The y chromaticity coordinate.
    pub y: f64,
}

/// The display color volume used to master the content.
///
/// Created from FFmpeg metadata only when both its primary-coordinate and
/// luminance flags are set. The named fields avoid exposing FFmpeg's array order
/// to the encoder formatters.
#[derive(Debug, Clone)]
pub struct MasteringDisplay {
    /// The red primary chromaticity.
    pub red: Point,
    /// The green primary chromaticity.
    pub green: Point,
    /// The blue primary chromaticity.
    pub blue: Point,
    /// The mastering display white-point chromaticity.
    pub white: Point,
    /// The minimum mastering display luminance, in cd/m².
    pub min_luminance: f64,
    /// The maximum mastering display luminance, in cd/m².
    pub max_luminance: f64,
}

/// Content light levels used by static HDR10 metadata.
#[derive(Debug, Clone, Copy)]
pub struct ContentLight {
    /// The maximum content light level (MaxCLL), in cd/m².
    pub max_cll: u32,
    /// The maximum frame-average light level (MaxFALL), in cd/m².
    pub max_fall: u32,
}

/// Color properties and static HDR10 records accumulated while scanning frames.
///
/// Use [`Self::empty`] for a scan that starts without stream color properties.
/// The derived [`Default`] instead initializes the color strings to empty
/// strings, which are not the `"unknown"` sentinel used by the frame updater.
#[derive(Debug, Default)]
pub struct HdrMetadata {
    /// The matrix coefficients, represented by an FFmpeg debug name.
    pub color_space: String,
    /// The color primaries, represented by an FFmpeg debug name.
    pub color_primaries: String,
    /// The transfer characteristic, represented by an FFmpeg debug name.
    pub color_transfer: String,
    /// The first complete mastering-display record, if found.
    pub mastering: Option<MasteringDisplay>,
    /// The first content-light record, if found.
    pub cll: Option<ContentLight>,
    /// The number of frames passed to the updater, including a failing update.
    pub frames_scanned: usize,
}

use std::os::raw::{
    c_int,
    c_uint,
};

/// The native mastering-display payload from FFmpeg's HDR metadata header.
///
/// `repr(C)` preserves field ordering and native C alignment. All fields are
/// integers or [`ffi::AVRational`] values; no Rust references are stored here.
#[repr(C)]
#[derive(Clone, Copy)]
struct AVMasteringDisplayMetadata {
    /// Chromaticity pairs in FFmpeg order: red, green, blue.
    display_primaries: [[ffi::AVRational; 2]; 3],
    /// The white-point chromaticity pair.
    white_point: [ffi::AVRational; 2],
    /// The minimum luminance rational, in cd/m².
    min_luminance: ffi::AVRational,
    /// The maximum luminance rational, in cd/m².
    max_luminance: ffi::AVRational,
    /// A nonzero value marks primaries and white point as present.
    has_primaries: c_int,
    /// A nonzero value marks both luminance values as present.
    has_luminance: c_int,
}

/// The native content-light payload from FFmpeg's HDR metadata header.
///
/// Field names follow the C header to make layout review straightforward.
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case)]
struct AVContentLightMetadata {
    /// The maximum content light level, in cd/m².
    MaxCLL: c_uint,
    /// The maximum frame-average light level, in cd/m².
    MaxFALL: c_uint,
}

/// Converts a native FFmpeg rational to a floating-point value.
///
/// # Errors
///
/// Returns an error if the denominator is zero. Negative values are accepted.
fn rational(r: ffi::AVRational) -> Result<f64> {
    if r.den == 0 {
        return Err("Invalid HDR rational: zero denominator".into());
    }
    Ok(f64::from(r.num) / f64::from(r.den))
}

/// Converts a pair of native rationals to chromaticity coordinates.
///
/// # Errors
///
/// Returns an error if either coordinate has a zero denominator.
fn point(p: [ffi::AVRational; 2]) -> Result<Point> {
    Ok(Point {
        x: rational(p[0])?,
        y: rational(p[1])?,
    })
}

/// Copies a native payload without requiring its buffer to be aligned for `T`.
///
/// # Errors
///
/// Returns an error when `bytes` is shorter than the native size of `T`.
///
/// # Safety
///
/// If the buffer is large enough, its first `size_of::<T>()` bytes must represent
/// a valid `T`, including any validity requirements of its fields. The payload
/// layout and native byte order must match `T`. `Copy` alone does not establish
/// these requirements.
unsafe fn read_payload<T: Copy>(bytes: &[u8]) -> Result<T> {
    if bytes.len() < size_of::<T>() {
        return Err("Truncated FFmpeg HDR side data".into());
    }
    // SAFETY: the length check makes the entire read fall within the live slice.
    // read_unaligned removes the alignment requirement. The caller guarantees
    // a valid T representation, and Copy prevents duplicating owned resources.
    Ok(unsafe { ptr::read_unaligned(bytes.as_ptr().cast::<T>()) })
}

impl HdrMetadata {
    /// Accumulates color properties and static HDR10 records from a video frame.
    ///
    /// Increments the scan count before reading side data. Color properties are
    /// replaced only when their string is exactly `"unknown"`. Existing HDR
    /// records are retained; unrelated side-data types are ignored.
    ///
    /// A mastering-display record is accepted only when both presence flags are
    /// set. Incomplete records are skipped so a later frame can provide one.
    ///
    /// # Errors
    ///
    /// Returns an error for a truncated recognized payload or a zero denominator
    /// in an accepted mastering-display record. Earlier updates to the scan count,
    /// color properties, or metadata are not rolled back on failure.
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
            // SAFETY: this FFmpeg side-data tag identifies the native mastering
            // payload. The mirrored C layout must match the linked library, and
            // its integer/rational fields permit every initialized bit pattern.
            let raw: AVMasteringDisplayMetadata = unsafe { read_payload(side.data())? };
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
            // SAFETY: this tag identifies the native content-light payload. Its
            // mirrored C layout must match the linked library, and both unsigned
            // fields permit every initialized bit pattern.
            let raw: AVContentLightMetadata = unsafe { read_payload(side.data())? };
            self.cll = Some(ContentLight {
                max_cll: raw.MaxCLL,
                max_fall: raw.MaxFALL,
            });
        }
        Ok(())
    }

    /// Returns whether both static HDR10 records have been found.
    ///
    /// Color properties and scan count do not affect completeness. This does not
    /// classify the input's transfer function or detect dynamic HDR metadata.
    pub fn complete(&self) -> bool {
        self.mastering.is_some() && self.cll.is_some()
    }
}

impl Default for Point {
    /// Returns zero chromaticity coordinates.
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl HdrMetadata {
    /// Returns an unscanned record with color properties set to `"unknown"`.
    ///
    /// Both HDR records are absent and the frame count is zero. Unlike the derived
    /// [`Default`], these color strings can be filled from frame properties.
    pub fn empty() -> Self {
        Self {
            color_space: "unknown".into(),
            color_primaries: "unknown".into(),
            color_transfer: "unknown".into(),
            ..Self::default()
        }
    }
}
