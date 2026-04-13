//! Helpers for writing text to the terminal clipboard via the OSC 52
//! escape sequence. Works with tmux (`set -g set-clipboard on|external`)
//! and lets paste succeed even in remote shells where `arboard` only
//! talks to the SSH server's local clipboard.

/// Build the OSC 52 escape sequence that tells the terminal to copy
/// `text` into the "c" (clipboard) selection. Terminals that don't
/// understand the sequence simply ignore it.
pub fn osc52_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64_encode(text.as_bytes()))
}

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(CHARS[(b0 >> 2) as usize] as char);
        out.push(CHARS[(((b0 & 0b11) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[(((b1 & 0b1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(b2 & 0b111111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_empty_input() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn base64_known_vectors_from_rfc4648() {
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn osc52_sequence_wraps_base64_payload() {
        let seq = osc52_sequence("foo");
        assert_eq!(seq, "\x1b]52;c;Zm9v\x07");
    }

    #[test]
    fn osc52_sequence_handles_unicode_text() {
        let seq = osc52_sequence("こんにちは");
        // UTF-8 bytes of "こんにちは" = E3 81 93 E3 82 93 E3 81 AB E3 81 A1 E3 81 AF
        assert!(seq.starts_with("\x1b]52;c;"));
        assert!(seq.ends_with('\x07'));
    }
}
