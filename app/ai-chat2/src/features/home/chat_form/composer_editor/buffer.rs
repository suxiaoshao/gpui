use std::ops::Range;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Selection {
    pub(super) anchor: usize,
    pub(super) head: usize,
}

impl Selection {
    pub(super) fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub(super) fn reversed(&self) -> bool {
        self.head < self.anchor
    }

    pub(super) fn collapse(&mut self, offset: usize) {
        self.anchor = offset;
        self.head = offset;
    }

    pub(super) fn move_head(&mut self, offset: usize, selecting: bool) {
        if selecting {
            self.head = offset;
        } else {
            self.collapse(offset);
        }
    }
}

pub(super) fn clamp_offset(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

pub(super) fn normalize_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start = clamp_offset(text, range.start);
    let end = clamp_offset(text, range.end);
    start.min(end)..start.max(end)
}

pub(super) fn utf16_to_byte(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0;
    let mut byte_offset = 0;

    for ch in text.chars() {
        if utf16_count >= utf16_offset {
            break;
        }
        utf16_count += ch.len_utf16();
        byte_offset += ch.len_utf8();
    }

    byte_offset
}

pub(super) fn byte_to_utf16(text: &str, byte_offset: usize) -> usize {
    let byte_offset = clamp_offset(text, byte_offset);
    let mut utf16_offset = 0;
    let mut byte_count = 0;

    for ch in text.chars() {
        if byte_count >= byte_offset {
            break;
        }
        utf16_offset += ch.len_utf16();
        byte_count += ch.len_utf8();
    }

    utf16_offset
}

pub(super) fn utf16_range_to_byte_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start = utf16_to_byte(text, range.start);
    let end = utf16_to_byte(text, range.end);
    normalize_range(text, start..end)
}

pub(super) fn byte_range_to_utf16_range(text: &str, range: Range<usize>) -> Range<usize> {
    byte_to_utf16(text, range.start)..byte_to_utf16(text, range.end)
}

pub(super) fn previous_char_boundary(text: &str, offset: usize) -> usize {
    let offset = clamp_offset(text, offset);
    if offset == 0 {
        return 0;
    }
    text[..offset]
        .char_indices()
        .last()
        .map(|(ix, _)| ix)
        .unwrap_or(0)
}

pub(super) fn next_char_boundary(text: &str, offset: usize) -> usize {
    let offset = clamp_offset(text, offset);
    if offset >= text.len() {
        return text.len();
    }
    text[offset..]
        .char_indices()
        .nth(1)
        .map(|(ix, _)| offset + ix)
        .unwrap_or(text.len())
}

pub(super) fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '$')
}

pub(super) fn previous_word_start(text: &str, offset: usize) -> usize {
    let mut cursor = clamp_offset(text, offset);
    if cursor == 0 {
        return 0;
    }

    while cursor > 0 {
        let prev = previous_char_boundary(text, cursor);
        let ch = text[prev..cursor].chars().next().unwrap();
        if !ch.is_whitespace() {
            break;
        }
        cursor = prev;
    }

    if cursor == 0 {
        return 0;
    }

    let prev = previous_char_boundary(text, cursor);
    let word_mode = text[prev..cursor].chars().next().is_some_and(is_word_char);
    while cursor > 0 {
        let prev = previous_char_boundary(text, cursor);
        let ch = text[prev..cursor].chars().next().unwrap();
        if is_word_char(ch) != word_mode || ch.is_whitespace() {
            break;
        }
        cursor = prev;
    }

    cursor
}

pub(super) fn next_word_end(text: &str, offset: usize) -> usize {
    let mut cursor = clamp_offset(text, offset);
    if cursor >= text.len() {
        return text.len();
    }

    while cursor < text.len() {
        let next = next_char_boundary(text, cursor);
        let ch = text[cursor..next].chars().next().unwrap();
        if !ch.is_whitespace() {
            break;
        }
        cursor = next;
    }

    if cursor >= text.len() {
        return text.len();
    }

    let next = next_char_boundary(text, cursor);
    let word_mode = text[cursor..next].chars().next().is_some_and(is_word_char);
    while cursor < text.len() {
        let next = next_char_boundary(text, cursor);
        let ch = text[cursor..next].chars().next().unwrap();
        if is_word_char(ch) != word_mode || ch.is_whitespace() {
            break;
        }
        cursor = next;
    }

    cursor
}

pub(super) fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;

    for (ix, ch) in text.char_indices() {
        if ch == '\n' {
            ranges.push(start..ix);
            start = ix + ch.len_utf8();
        }
    }

    ranges.push(start..text.len());
    ranges
}

pub(super) fn line_index_for_offset(text: &str, offset: usize) -> usize {
    let offset = clamp_offset(text, offset);
    let ranges = line_ranges(text);
    ranges
        .iter()
        .position(|range| offset <= range.end)
        .unwrap_or_else(|| ranges.len().saturating_sub(1))
}

pub(super) fn line_start(text: &str, offset: usize) -> usize {
    let ranges = line_ranges(text);
    ranges
        .get(line_index_for_offset(text, offset))
        .map_or(0, |range| range.start)
}

pub(super) fn line_end(text: &str, offset: usize) -> usize {
    let ranges = line_ranges(text);
    ranges
        .get(line_index_for_offset(text, offset))
        .map_or(text.len(), |range| range.end)
}

pub(super) fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let offset = clamp_offset(text, offset);
    let line_ix = line_index_for_offset(text, offset);
    let start = line_ranges(text)
        .get(line_ix)
        .map_or(0, |range| range.start);
    (line_ix, offset.saturating_sub(start))
}

pub(super) fn offset_for_line_column(text: &str, line_ix: usize, column: usize) -> usize {
    let ranges = line_ranges(text);
    let range = ranges
        .get(line_ix.min(ranges.len().saturating_sub(1)))
        .cloned()
        .unwrap_or(0..0);
    clamp_offset(text, (range.start + column).min(range.end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_roundtrip_clips_to_char_boundaries() {
        let text = "a你🙂b";
        for offset in [0, 1, 4, 8, text.len()] {
            assert_eq!(utf16_to_byte(text, byte_to_utf16(text, offset)), offset);
        }
        assert_eq!(utf16_to_byte(text, 999), text.len());
        assert_eq!(byte_to_utf16(text, 999), 5);
    }

    #[test]
    fn line_ranges_keep_trailing_empty_line() {
        assert_eq!(line_ranges("a\nb"), vec![0..1, 2..3]);
        assert_eq!(line_ranges("a\n"), vec![0..1, 2..2]);
    }

    #[test]
    fn word_boundaries_treat_skill_name_as_word() {
        let text = "ask $rust-skill now";
        assert_eq!(previous_word_start(text, 15), 4);
        assert_eq!(next_word_end(text, 4), 15);
    }
}
