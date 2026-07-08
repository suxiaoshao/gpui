use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

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

pub(super) fn is_grapheme_boundary(text: &str, offset: usize) -> bool {
    let offset = clamp_offset(text, offset);
    offset == 0
        || offset == text.len()
        || UnicodeSegmentation::grapheme_indices(text, true).any(|(start, _)| start == offset)
}

pub(super) fn clamp_grapheme_offset(text: &str, offset: usize) -> usize {
    let offset = clamp_offset(text, offset);
    if offset == 0 || offset == text.len() {
        return offset;
    }

    for (start, grapheme) in UnicodeSegmentation::grapheme_indices(text, true) {
        let end = start + grapheme.len();
        if offset == start || offset == end {
            return offset;
        }
        if offset > start && offset < end {
            return if offset - start <= end - offset {
                start
            } else {
                end
            };
        }
    }

    offset
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

pub(super) fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    let offset = clamp_offset(text, offset);
    if offset == 0 {
        return 0;
    }

    UnicodeSegmentation::grapheme_indices(text, true)
        .take_while(|(start, _)| *start < offset)
        .last()
        .map(|(start, _)| start)
        .unwrap_or(0)
}

pub(super) fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    let offset = clamp_offset(text, offset);
    if offset >= text.len() {
        return text.len();
    }

    UnicodeSegmentation::grapheme_indices(text, true)
        .find_map(|(start, grapheme)| {
            let end = start + grapheme.len();
            (end > offset).then_some(end)
        })
        .unwrap_or(text.len())
}

#[derive(Clone, Debug)]
struct WordPart {
    range: Range<usize>,
    kind: WordPartKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WordPartKind {
    Whitespace,
    Connector,
    Word,
    Other,
}

fn is_word_connector(ch: char) -> bool {
    matches!(ch, '$' | '_' | '-')
}

pub(super) fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || is_word_connector(ch)
}

fn word_part_kind(segment: &str) -> WordPartKind {
    if segment.chars().all(char::is_whitespace) {
        WordPartKind::Whitespace
    } else if segment.chars().all(is_word_connector) {
        WordPartKind::Connector
    } else if segment.chars().all(is_word_char) {
        WordPartKind::Word
    } else {
        WordPartKind::Other
    }
}

fn word_parts(text: &str) -> Vec<WordPart> {
    UnicodeSegmentation::split_word_bound_indices(text)
        .map(|(start, segment)| WordPart {
            range: start..start + segment.len(),
            kind: word_part_kind(segment),
        })
        .collect()
}

fn word_chunks(text: &str) -> Vec<Range<usize>> {
    let parts = word_parts(text);
    let mut chunks = Vec::new();
    let mut ix = 0;

    while ix < parts.len() {
        if parts[ix].kind == WordPartKind::Whitespace {
            ix += 1;
            continue;
        }

        let start = parts[ix].range.start;
        let mut end = parts[ix].range.end;
        let mut last_kind = parts[ix].kind;
        ix += 1;

        while ix < parts.len() && parts[ix].kind != WordPartKind::Whitespace {
            let joins_word = last_kind == WordPartKind::Connector
                || parts[ix].kind == WordPartKind::Connector
                || (last_kind == WordPartKind::Word && parts[ix].kind == WordPartKind::Word)
                || !is_grapheme_boundary(text, parts[ix].range.start);
            if !joins_word {
                break;
            }

            end = parts[ix].range.end;
            last_kind = parts[ix].kind;
            ix += 1;
        }

        chunks.push(start..end);
    }

    chunks
}

pub(super) fn word_range_at(text: &str, offset: usize) -> Option<Range<usize>> {
    if text.is_empty() {
        return None;
    }

    let offset = clamp_grapheme_offset(text, offset);
    let target = if offset == text.len() {
        previous_grapheme_boundary(text, offset)
    } else {
        offset
    };

    word_chunks(text)
        .into_iter()
        .find(|range| target >= range.start && target < range.end)
}

pub(super) fn previous_word_start(text: &str, offset: usize) -> usize {
    let offset = clamp_grapheme_offset(text, offset);
    if offset == 0 {
        return 0;
    }

    for range in word_chunks(text).into_iter().rev() {
        if (range.start < offset && offset <= range.end) || range.end < offset {
            return range.start;
        }
    }

    0
}

pub(super) fn next_word_end(text: &str, offset: usize) -> usize {
    let offset = clamp_grapheme_offset(text, offset);
    if offset >= text.len() {
        return text.len();
    }

    for range in word_chunks(text) {
        if range.end > offset {
            return range.end;
        }
    }

    text.len()
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
    fn grapheme_boundaries_treat_clusters_as_single_units() {
        let acute = "e\u{301}";
        let flag = "🇨🇳";
        let coder = "👩🏽‍💻";
        let family = "👨‍👩‍👧‍👦";
        let text = format!("{acute}{flag}{coder}{family}");
        let offsets = [
            0,
            acute.len(),
            acute.len() + flag.len(),
            acute.len() + flag.len() + coder.len(),
            text.len(),
        ];

        for window in offsets.windows(2) {
            assert_eq!(next_grapheme_boundary(&text, window[0]), window[1]);
            assert_eq!(previous_grapheme_boundary(&text, window[1]), window[0]);
        }

        let inside_acute = "e".len();
        assert_eq!(previous_grapheme_boundary(&text, inside_acute), 0);
        assert_eq!(next_grapheme_boundary(&text, inside_acute), acute.len());
        assert_eq!(clamp_grapheme_offset(&text, inside_acute), 0);

        let coder_start = acute.len() + flag.len();
        let inside_coder = coder_start + "👩".len();
        assert_eq!(previous_grapheme_boundary(&text, inside_coder), coder_start);
        assert_eq!(
            next_grapheme_boundary(&text, inside_coder),
            coder_start + coder.len()
        );
        assert_eq!(clamp_grapheme_offset(&text, inside_coder), coder_start);
    }

    #[test]
    fn word_boundaries_treat_skill_name_as_word() {
        let text = "ask $rust-skill now";
        assert_eq!(previous_word_start(text, 15), 4);
        assert_eq!(next_word_end(text, 4), 15);
        assert_eq!(word_range_at(text, 5), Some(4..15));
        assert_eq!(word_range_at(text, 3), None);
    }

    #[test]
    fn word_boundaries_follow_unicode_segments() {
        let text = "hi 中文 👩🏽‍💻 !";
        let chinese_start = text.find("中文").unwrap();
        let emoji_start = text.find("👩").unwrap();
        let bang_start = text.find('!').unwrap();

        assert_eq!(
            word_range_at(text, chinese_start),
            Some(chinese_start..chinese_start + "中文".len())
        );
        assert_eq!(
            word_range_at(text, emoji_start),
            Some(emoji_start..emoji_start + "👩🏽‍💻".len())
        );
        assert_eq!(
            word_range_at(text, bang_start),
            Some(bang_start..bang_start + 1)
        );
        assert_eq!(word_range_at(text, text.find(' ').unwrap()), None);

        assert_eq!(
            next_word_end(text, chinese_start),
            chinese_start + "中文".len()
        );
        assert_eq!(
            previous_word_start(text, emoji_start + "👩🏽‍💻".len()),
            emoji_start
        );
    }
}
