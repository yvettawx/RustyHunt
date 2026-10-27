//! Color parsing: `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r,g,b)`, `rgba(r,g,b,a)`.

use egui::Color32;

pub fn parse(s: &str) -> Option<Color32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let v = |i: usize, n: usize| u8::from_str_radix(&hex[i..i + n], 16).ok();
        return match hex.len() {
            3 => Some(Color32::from_rgb(v(0, 1)? * 17, v(1, 1)? * 17, v(2, 1)? * 17)),
            6 => Some(Color32::from_rgb(v(0, 2)?, v(2, 2)?, v(4, 2)?)),
            8 => Some(Color32::from_rgba_unmultiplied(v(0, 2)?, v(2, 2)?, v(4, 2)?, v(6, 2)?)),
            _ => None,
        };
    }
    let (name, args) = s.split_once('(')?;
    let args: Vec<f32> = args.trim_end_matches(')').split(',').map(|a| a.trim().parse().ok()).collect::<Option<_>>()?;
    let c = |x: f32| x.round().clamp(0.0, 255.0) as u8;
    match (name.trim(), args.as_slice()) {
        ("rgb", [r, g, b]) => Some(Color32::from_rgb(c(*r), c(*g), c(*b))),
        ("rgba", [r, g, b, a]) => Some(Color32::from_rgba_unmultiplied(c(*r), c(*g), c(*b), c(a * 255.0))),
        _ => None,
    }
}

pub fn to_hex(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

/// Multiplies by an opacity.
pub fn fade(c: Color32, alpha: f32) -> Color32 {
    c.gamma_multiply(alpha.clamp(0.0, 1.0))
}

pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied_const(r, g, b, (a * 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_forms() {
        assert_eq!(parse("#ff6b1a"), Some(Color32::from_rgb(0xff, 0x6b, 0x1a)));
        assert_eq!(parse("#fff"), Some(Color32::WHITE));
        assert_eq!(parse("rgb(17, 17, 17)"), Some(Color32::from_rgb(17, 17, 17)));
        assert_eq!(parse("rgba(0,0,0,.5)"), Some(Color32::from_rgba_unmultiplied(0, 0, 0, 128)));
        assert_eq!(parse("#12345"), None);
        assert_eq!(parse("blue"), None);
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(to_hex(parse("#ff6b1a").unwrap()), "#ff6b1a");
        // Translucent colors are premultiplied internally, so a round trip is only approximately equal
        let back = parse(&to_hex(parse("#80808080").unwrap())).unwrap();
        assert!((back.a() as i32 - 0x80).abs() <= 1);
    }
}
