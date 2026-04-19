/// Standard xterm 256-color palette. Index 0-15 are the classic ANSI
/// colors; 16-231 form a 6x6x6 color cube; 232-255 are a grayscale ramp.
pub(crate) fn ansi_256_to_rgb(n: u8) -> u32 {
    const BASIC: [u32; 16] = [
        0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xc0c0c0, 0x808080,
        0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff,
    ];
    if (n as usize) < 16 {
        return BASIC[n as usize];
    }
    if n >= 232 {
        let v = 8 + 10 * (n - 232) as u32;
        return (v << 16) | (v << 8) | v;
    }
    let idx = n - 16;
    let r = idx / 36;
    let g = (idx % 36) / 6;
    let b = idx % 6;
    let to = |c: u8| -> u32 {
        if c == 0 {
            0
        } else {
            (55 + 40 * c as u32) & 0xff
        }
    };
    (to(r) << 16) | (to(g) << 8) | to(b)
}

pub(crate) fn push_escaped(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            ' ' => out.push_str("&nbsp;"),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_256_basic_range() {
        assert_eq!(ansi_256_to_rgb(0), 0x000000);
        assert_eq!(ansi_256_to_rgb(15), 0xffffff);
    }

    #[test]
    fn ansi_256_cube_range() {
        // Index 16 is (r=0,g=0,b=0) in the cube = pure black.
        assert_eq!(ansi_256_to_rgb(16), 0x000000);
        // Index 231 is the top of the cube — pure white.
        assert_eq!(ansi_256_to_rgb(231), 0xffffff);
    }

    #[test]
    fn ansi_256_gray_ramp() {
        assert_eq!(ansi_256_to_rgb(232), 0x080808);
        assert_eq!(ansi_256_to_rgb(255), 0xeeeeee);
    }

    #[test]
    fn push_escaped_escapes_html_specials() {
        let mut s = String::new();
        push_escaped(&mut s, "<a & \"b\">");
        assert_eq!(s, "&lt;a&nbsp;&amp;&nbsp;&quot;b&quot;&gt;");
    }
}
