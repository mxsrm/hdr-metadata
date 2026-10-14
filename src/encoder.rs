use crate::metadata::{ContentLight, HdrMetadata, MasteringDisplay, Point};

fn scaled_point(p: Point) -> String {
    format!("{:.0},{:.0}", (p.x * 50_000.0).round(), (p.y * 50_000.0).round())
}

fn decimal_point(p: Point) -> String { format!("{:.4},{:.4}", p.x, p.y) }

pub fn x265(display: &MasteringDisplay, cll: Option<ContentLight>) -> String {
    let mut result = format!(
        "master-display=G({})B({})R({})WP({})L({:.0},{:.0})",
        scaled_point(display.green), scaled_point(display.blue),
        scaled_point(display.red), scaled_point(display.white),
        (display.max_luminance * 10_000.0).round(),
        (display.min_luminance * 10_000.0).round(),
    );
    if let Some(c) = cll {
        result.push_str(&format!(":max-cll={},{}", c.max_cll, c.max_fall));
    }
    result
}

pub fn svt_av1(display: &MasteringDisplay, cll: Option<ContentLight>) -> String {
    let mut result = format!(
        "mastering-display=G({})B({})R({})WP({})L({:.4},{:.4})",
        decimal_point(display.green), decimal_point(display.blue),
        decimal_point(display.red), decimal_point(display.white),
        display.max_luminance, display.min_luminance,
    );
    if let Some(c) = cll {
        result.push_str(&format!(":content-light={},{}", c.max_cll, c.max_fall));
    }
    result
}

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
    if hdr.cll.is_none() { println!("\nNo content light level metadata found."); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bt2020_mastering_format() {
        let d = MasteringDisplay {
            green: Point { x: 0.170, y: 0.797 },
            blue: Point { x: 0.131, y: 0.046 },
            red: Point { x: 0.708, y: 0.292 },
            white: Point { x: 0.3127, y: 0.329 },
            max_luminance: 1000.0, min_luminance: 0.005,
        };
        let cll = Some(ContentLight { max_cll: 1000, max_fall: 400 });
        assert_eq!(x265(&d, cll), "master-display=G(8500,39850)B(6550,2300)R(35400,14600)WP(15635,16450)L(10000000,50):max-cll=1000,400");
        assert!(svt_av1(&d, cll).contains("content-light=1000,400"));
    }
}
