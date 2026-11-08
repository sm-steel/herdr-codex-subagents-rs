//! Terminal-safe display text contract.

/// Remove control sequences and bound UTF-8 display text.
#[must_use]
pub fn display_text(input: &str, max_bytes: usize) -> String {
    let mut output = String::new();
    let mut state = EscapeState::Text;
    let mut whitespace = false;
    for ch in input.chars() {
        match state {
            EscapeState::Escape => {
                state = match ch {
                    '[' => EscapeState::Csi,
                    ']' | 'P' | '^' | '_' => EscapeState::String,
                    _ => EscapeState::Text,
                };
                continue;
            }
            EscapeState::Csi => {
                if ('\u{40}'..='\u{7e}').contains(&ch) {
                    state = EscapeState::Text;
                }
                continue;
            }
            EscapeState::String => {
                if ch == '\u{7}' || ch == '\u{9c}' {
                    state = EscapeState::Text;
                } else if ch == '\u{1b}' {
                    state = EscapeState::StringEscape;
                }
                continue;
            }
            EscapeState::StringEscape => {
                state = if ch == '\\' {
                    EscapeState::Text
                } else {
                    EscapeState::String
                };
                continue;
            }
            EscapeState::Text => {}
        }
        match ch {
            '\u{1b}' => {
                state = EscapeState::Escape;
                continue;
            }
            '\u{9b}' => {
                state = EscapeState::Csi;
                continue;
            }
            '\u{9d}' | '\u{90}' | '\u{9e}' | '\u{9f}' => {
                state = EscapeState::String;
                continue;
            }
            _ => {}
        }
        if ch.is_whitespace() {
            whitespace = !output.is_empty();
            continue;
        }
        if ch.is_control() {
            continue;
        }
        let space_bytes = usize::from(whitespace);
        if output.len() + space_bytes + ch.len_utf8() > max_bytes {
            break;
        }
        if whitespace {
            output.push(' ');
            whitespace = false;
        }
        output.push(ch);
    }
    output
}

enum EscapeState {
    Text,
    Escape,
    Csi,
    String,
    StringEscape,
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
