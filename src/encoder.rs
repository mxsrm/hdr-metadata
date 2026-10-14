//! Formats static HDR10 metadata for x265 and SVT-AV1.
//!
//! Both formats order display primaries as green, blue, red, followed by the
//! white point and maximum/minimum luminance. These functions emit parameter
//! strings; they do not launch an encoder or validate its configuration.

use crate::metadata::{
    ContentLight,
    HdrMetadata,
    MasteringDisplay,
    Point,
};

/// Converts chromaticity coordinates to x265's units of 1/50,000.
///
/// Rounds only after scaling, preserving precision until the final conversion.
fn scaled_point(p: Point) -> String {
    format!("{:.0},{:.0}", (p.x * 50_000.0).round(), (p.y * 50_000.0).round())
}

/// Formats chromaticity coordinates with four decimal places.
fn decimal_point(p: Point) -> String {
    format!("{:.4},{:.4}", p.x, p.y)
}

/// Returns x265 parameters for the mastering display and optional light levels.
///
/// Chromaticity coordinates are scaled by 50,000; luminance in cd/m² is scaled
/// by 10,000. Values are rounded to the nearest integer, with halfway cases
/// rounded away from zero. `cll` adds `max-cll` only when present.
///
/// The input is assumed to describe a valid display; this function does not
/// reject out-of-range or non-finite values.
///
/// # Example
///
/// A BT.2020 display with a D65 white point, luminance of 0.005-1,000 cd/m²,
/// MaxCLL of 1,000 cd/m², and MaxFALL of 400 cd/m² produces:
///
/// ```text
/// master-display=G(8500,39850)B(6550,2300)R(35400,14600)WP(15635,16450)L(10000000,50):max-cll=1000,400
/// ```
pub fn x265(display: &MasteringDisplay, cll: Option<ContentLight>) -> String {
    let mut result = format!(
        "master-display=G({})B({})R({})WP({})L({:.0},{:.0})",
        scaled_point(display.green),
        scaled_point(display.blue),
        scaled_point(display.red),
        scaled_point(display.white),
        (display.max_luminance * 10_000.0).round(),
        (display.min_luminance * 10_000.0).round(),
    );
    if let Some(c) = cll {
        result.push_str(&format!(":max-cll={},{}", c.max_cll, c.max_fall));
    }
    result
}

/// Returns SVT-AV1 parameters using decimal display coordinates and luminance.
///
/// Values are formatted with four decimal places. `cll` adds `content-light`
/// only when present. As with [`x265`], display values are not validated here.
///
/// # Example
///
/// For the display described in [`x265`], the decimal representation is:
///
/// ```text
/// mastering-display=G(0.1700,0.7970)B(0.1310,0.0460)R(0.7080,0.2920)WP(0.3127,0.3290)L(1000.0000,0.0050):content-light=1000,400
/// ```
pub fn svt_av1(display: &MasteringDisplay, cll: Option<ContentLight>) -> String {
    let mut result = format!(
        "mastering-display=G({})B({})R({})WP({})L({:.4},{:.4})",
        decimal_point(display.green),
        decimal_point(display.blue),
        decimal_point(display.red),
        decimal_point(display.white),
        display.max_luminance,
        display.min_luminance,
    );
    if let Some(c) = cll {
        result.push_str(&format!(":content-light={},{}", c.max_cll, c.max_fall));
    }
    result
}

/// Prints the input path, scan count, color properties, and encoder parameters.
///
/// Writes to standard output. Encoder parameters are printed only when complete
/// mastering-display metadata is available. Missing metadata is reported in
/// prose; content-light metadata alone does not produce an encoder string.
///
/// # Panics
///
/// Panics if writing to standard output fails, as with [`println!`].
pub fn print_report(path: &std::path::Path, hdr: &HdrMetadata) {
    println!("\nFile: {}", path.display());
    println!("Frames scanned: {}\n", hdr.frames_scanned);
    println!("color_space:       {}", hdr.color_space);
    println!("color_primaries:   {}", hdr.color_primaries);
    println!("color_transfer:    {}", hdr.color_transfer);
    if let Some(d) = &hdr.mastering {
        println!("\nx265:\n{}", x265(d, hdr.cll));
        println!("\nSVT-AV1:\n{}", svt_av1(d, hdr.cll));
    } else {
        println!("\nNo complete mastering display metadata found.");
    }
    if hdr.cll.is_none() {
        println!("\nNo content light level metadata found.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Check primary ordering and integer scaling against a BT.2020 fixture.
    #[test]
    fn bt2020_mastering_format() {
        let d = MasteringDisplay {
            green: Point { x: 0.170, y: 0.797 },
            blue: Point { x: 0.131, y: 0.046 },
            red: Point { x: 0.708, y: 0.292 },
            white: Point { x: 0.3127, y: 0.329 },
            max_luminance: 1000.0,
            min_luminance: 0.005,
        };
        let cll = Some(ContentLight {
            max_cll: 1000,
            max_fall: 400,
        });
        assert_eq!(
            x265(&d, cll),
            "master-display=G(8500,39850)B(6550,2300)R(35400,14600)WP(15635,16450)L(10000000,50):max-cll=1000,400"
        );
        assert!(svt_av1(&d, cll).contains("content-light=1000,400"));
    }
}
