//! Bounded JSONL framing contract; implementation awaits test review.

/// A complete record or a diagnostic for a discarded oversized record.
#[derive(Debug, PartialEq, Eq)]
pub enum Record {
    /// Complete line bytes, excluding the newline delimiter and optional CR.
    Line(Vec<u8>),
    /// One oversized record was discarded through its next newline.
    Oversized,
}

/// Incremental byte framing; decoding happens only after an entire line arrives.
pub struct RecordBuffer;

impl RecordBuffer {
    /// Construct a framer with a maximum record byte length.
    #[must_use]
    pub const fn new(_max_record_bytes: usize) -> Self {
        Self
    }

    /// Feed new bytes, retaining a partial line between calls.
    #[must_use]
    pub fn push(&mut self, _bytes: &[u8]) -> Vec<Record> {
        Vec::new()
    }

    /// Forget an incomplete record after file truncation or replacement.
    pub fn reset(&mut self) {}

    /// Current retained bytes, bounded by the configured limit.
    #[must_use]
    pub const fn buffered_bytes(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_multiple_complete_records() {
        assert_eq!(
            RecordBuffer::new(100).push(b"one\ntwo\n"),
            vec![Record::Line(b"one".to_vec()), Record::Line(b"two".to_vec())]
        );
    }
    #[test]
    fn retains_partial_records_until_newline() {
        let mut buffer = RecordBuffer::new(100);
        assert!(buffer.push(b"partial").is_empty());
        assert_eq!(buffer.buffered_bytes(), 7);
        assert_eq!(
            buffer.push(b" end\n"),
            vec![Record::Line(b"partial end".to_vec())]
        );
        assert_eq!(buffer.buffered_bytes(), 0);
    }
    #[test]
    fn utf8_can_be_split_at_every_byte_boundary() {
        let bytes = "{\"text\":\"Привет 🦀\"}\n".as_bytes();
        for boundary in 0..bytes.len() {
            let mut buffer = RecordBuffer::new(100);
            assert!(buffer.push(&bytes[..boundary]).is_empty());
            assert_eq!(
                buffer.push(&bytes[boundary..]),
                vec![Record::Line(bytes[..bytes.len() - 1].to_vec())]
            );
        }
    }
    #[test]
    fn accepts_lf_and_crlf_without_trailing_carriage_return() {
        assert_eq!(
            RecordBuffer::new(100).push(b"one\r\ntwo\n"),
            vec![Record::Line(b"one".to_vec()), Record::Line(b"two".to_vec())]
        );
    }
    #[test]
    fn accepts_a_record_exactly_at_limit() {
        assert_eq!(
            RecordBuffer::new(4).push(b"1234\n"),
            vec![Record::Line(b"1234".to_vec())]
        );
    }
    #[test]
    fn discards_oversized_record_and_recovers_at_next_newline() {
        assert_eq!(
            RecordBuffer::new(4).push(b"12345\nok\n"),
            vec![Record::Oversized, Record::Line(b"ok".to_vec())]
        );
    }
    #[test]
    fn fragmented_oversize_generates_only_one_diagnostic() {
        let mut buffer = RecordBuffer::new(4);
        assert!(buffer.push(b"1234").is_empty());
        let mut events = buffer.push(b"5678");
        events.extend(buffer.push(b"90\nok\n"));
        assert_eq!(
            events,
            vec![Record::Oversized, Record::Line(b"ok".to_vec())]
        );
        assert!(buffer.buffered_bytes() <= 4);
    }
    #[test]
    fn oversized_partial_record_never_grows_without_bound() {
        let mut buffer = RecordBuffer::new(4);
        let _ = buffer.push(&[b'x'; 4096]);
        assert!(buffer.buffered_bytes() <= 4);
        assert_eq!(
            buffer.push(b"\nnext\n").last(),
            Some(&Record::Line(b"next".to_vec()))
        );
    }
    #[test]
    fn reset_prevents_joining_old_and_replaced_file_contents() {
        let mut buffer = RecordBuffer::new(100);
        assert!(buffer.push(b"old-").is_empty());
        buffer.reset();
        assert_eq!(buffer.push(b"new\n"), vec![Record::Line(b"new".to_vec())]);
    }
    #[test]
    fn reset_also_clears_oversize_discard_mode() {
        let mut buffer = RecordBuffer::new(4);
        let _ = buffer.push(b"too long");
        buffer.reset();
        assert_eq!(buffer.push(b"new\n"), vec![Record::Line(b"new".to_vec())]);
    }
    #[test]
    fn unterminated_tail_is_not_a_complete_record() {
        let mut buffer = RecordBuffer::new(100);
        assert!(buffer.push(b"{\"partial\":true}").is_empty());
    }
}
