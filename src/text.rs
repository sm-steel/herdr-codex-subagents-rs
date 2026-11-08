//! Terminal-safe display text contract.

/// Remove control sequences and bound UTF-8 display text.
#[must_use]
pub fn display_text(_input: &str, _max_bytes: usize) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_ordinary_unicode() {
        assert_eq!(display_text("Привет 🦀", 100), "Привет 🦀");
    }
    #[test]
    fn strips_csi_colors_without_eating_text() {
        assert_eq!(display_text("a\x1b[31mred\x1b[0mz", 100), "aredz");
    }
    #[test]
    fn strips_osc_title_and_clipboard_sequences() {
        assert_eq!(
            display_text("a\x1b]52;c;PRIVATE\x07b\x1b]0;title\x1b\\c", 100),
            "abc"
        );
    }
    #[test]
    fn strips_unterminated_control_sequence() {
        assert_eq!(display_text("safe\x1b]52;c;unfinished", 100), "safe");
    }
    #[test]
    fn flattens_whitespace_and_removes_other_controls() {
        assert_eq!(display_text("a\n\r\tb\0\u{7f}c", 100), "a bc");
    }
    #[test]
    fn truncates_only_at_utf8_boundary() {
        assert_eq!(display_text("ab🦀c", 5), "ab");
        assert_eq!(display_text("ab🦀c", 6), "ab🦀");
    }
    #[test]
    fn strips_c1_terminal_controls() {
        assert_eq!(display_text("a\u{9b}31mred\u{9b}0mb", 100), "aredb");
    }
}
