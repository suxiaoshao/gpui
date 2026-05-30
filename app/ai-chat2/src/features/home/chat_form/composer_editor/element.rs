use gpui::{
    App, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity, GlobalElementId,
    Hsla, IntoElement, LayoutId, PaintQuad, Pixels, Point, Style, TextAlign, TextRun,
    UnderlineStyle, Window, WrappedLine, fill, point, px, relative, size,
};
use gpui_component::ActiveTheme;

use super::{ComposerEditor, blink_cursor::CURSOR_WIDTH, buffer, token::ComposerToken};

pub(super) struct ComposerEditorElement {
    editor: Entity<ComposerEditor>,
}

impl ComposerEditorElement {
    pub(super) fn new(editor: Entity<ComposerEditor>) -> Self {
        Self { editor }
    }
}

pub(super) struct LayoutLine {
    pub(super) range: std::ops::Range<usize>,
    pub(super) y: Pixels,
    pub(super) visual_row: usize,
    pub(super) visual_line_count: usize,
    pub(super) line: WrappedLine,
}

pub(super) struct LayoutCache {
    pub(super) bounds: Bounds<Pixels>,
    pub(super) viewport_bounds: Bounds<Pixels>,
    pub(super) line_height: Pixels,
    pub(super) visual_line_count: usize,
    pub(super) lines: Vec<LayoutLine>,
}

impl LayoutCache {
    pub(super) fn content_height(&self) -> Pixels {
        self.line_height * self.visual_line_count.max(1) as f32
    }

    pub(super) fn bounds_for_offset(&self, text: &str, offset: usize) -> Option<Bounds<Pixels>> {
        let offset = buffer::clamp_offset(text, offset);
        let line = self.line_for_offset(offset)?;
        let local_offset = offset
            .saturating_sub(line.range.start)
            .min(line.range.len());
        let position = line
            .line
            .position_for_index(local_offset, self.line_height)
            .unwrap_or_else(|| point(px(0.), px(0.)));

        Some(Bounds::new(
            point(
                self.bounds.left() + position.x,
                self.bounds.top() + line.y + position.y,
            ),
            size(px(1.), self.line_height),
        ))
    }

    pub(super) fn offset_for_position(&self, text: &str, position: Point<Pixels>) -> usize {
        if self.lines.is_empty() {
            return 0;
        }

        let local_y = position.y - self.bounds.top();
        let line = self.line_for_y(local_y);
        let line_height = self.line_height * line.visual_line_count.max(1) as f32;
        let point = point(
            position.x - self.bounds.left(),
            (local_y - line.y)
                .max(px(0.))
                .min(line_height - self.line_height),
        );
        let local_offset = line
            .line
            .closest_index_for_position(point, self.line_height)
            .unwrap_or_else(|offset| offset)
            .min(line.range.len());

        buffer::clamp_offset(text, line.range.start + local_offset)
    }

    pub(super) fn offset_for_vertical_move(
        &self,
        text: &str,
        offset: usize,
        delta: isize,
        preferred_x: Option<Pixels>,
    ) -> Option<(usize, Pixels)> {
        if self.lines.is_empty() {
            return Some((0, px(0.)));
        }

        let offset = buffer::clamp_offset(text, offset);
        let line = self.line_for_offset(offset)?;
        let local_offset = offset
            .saturating_sub(line.range.start)
            .min(line.range.len());
        let position = line
            .line
            .position_for_index(local_offset, self.line_height)
            .unwrap_or_else(|| point(px(0.), px(0.)));
        let preferred_x = preferred_x.unwrap_or(position.x);
        let current_visual_row = line.visual_row + (position.y / self.line_height).floor() as usize;
        let target_visual_row = if delta < 0 {
            current_visual_row.saturating_sub(delta.unsigned_abs())
        } else {
            (current_visual_row + delta as usize).min(self.visual_line_count.saturating_sub(1))
        };
        let target_line = self.line_for_visual_row(target_visual_row)?;
        let local_visual_row = target_visual_row.saturating_sub(target_line.visual_row);
        let local_y = self.line_height * local_visual_row as f32;
        let local_offset = target_line
            .line
            .closest_index_for_position(point(preferred_x, local_y), self.line_height)
            .unwrap_or_else(|offset| offset)
            .min(target_line.range.len());

        Some((
            buffer::clamp_offset(text, target_line.range.start + local_offset),
            preferred_x,
        ))
    }

    fn line_for_offset(&self, offset: usize) -> Option<&LayoutLine> {
        self.lines
            .iter()
            .find(|line| offset >= line.range.start && offset <= line.range.end)
            .or_else(|| self.lines.last())
    }

    fn line_for_y(&self, y: Pixels) -> &LayoutLine {
        let y = y.max(px(0.));
        self.lines
            .iter()
            .find(|line| {
                y >= line.y && y < line.y + self.line_height * line.visual_line_count as f32
            })
            .unwrap_or_else(|| self.lines.last().unwrap())
    }

    fn line_for_visual_row(&self, row: usize) -> Option<&LayoutLine> {
        self.lines
            .iter()
            .find(|line| {
                row >= line.visual_row && row < line.visual_row + line.visual_line_count.max(1)
            })
            .or_else(|| self.lines.last())
    }
}

pub(super) struct PrepaintState {
    lines: Vec<LayoutLine>,
    cursor: Option<Bounds<Pixels>>,
    selections: Vec<PaintQuad>,
}

impl IntoElement for ComposerEditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ComposerEditorElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let line_count = self.editor.read(cx).content_line_count();
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * line_count as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let editor = self.editor.read(cx);
        let text = editor.text().to_string();
        let selection = editor.selection().range();
        let cursor = editor.selection().head;
        let marked_range = editor.marked_range().clone();
        let tokens = editor.tokens().to_vec();
        let placeholder = editor.placeholder().clone();
        let show_cursor = editor.show_cursor(window, cx);
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let cursor_height = font_size.min(line_height);
        let base_color = text_style.color;
        let placeholder_color = cx.theme().muted_foreground.opacity(0.72);
        let token_color = cx.theme().blue;
        let selection_color = cx.theme().blue.opacity(0.22);

        let ranges = buffer::line_ranges(&text);
        let is_placeholder = text.is_empty();
        let mut lines = Vec::with_capacity(ranges.len());
        let mut selections = Vec::new();
        let mut cursor_quad = None;
        let mut visual_row = 0;
        let wrap_width = bounds.size.width.max(px(1.));

        for range in ranges.into_iter() {
            let y = line_height * visual_row as f32;
            let (line_text, runs) = if is_placeholder {
                (
                    placeholder.to_string(),
                    vec![text_run(placeholder.len(), placeholder_color, None)],
                )
            } else if range.is_empty() {
                (
                    " ".to_string(),
                    vec![text_run(1, base_color.opacity(0.), None)],
                )
            } else {
                let line_text = text[range.clone()].to_string();
                let runs = line_runs(
                    &text,
                    range.clone(),
                    &tokens,
                    marked_range.clone(),
                    base_color,
                    token_color,
                );
                (line_text, runs)
            };

            let mut wrapped_lines = window
                .text_system()
                .shape_text(line_text.into(), font_size, &runs, Some(wrap_width), None)
                .unwrap_or_default();
            let line = wrapped_lines.pop().unwrap_or_default();
            let visual_line_count = line.wrap_boundaries().len() + 1;
            let line_origin = point(bounds.left(), bounds.top() + y);

            if !selection.is_empty() && !is_placeholder {
                push_selection_quads(
                    &mut selections,
                    selection.clone(),
                    range.clone(),
                    &line,
                    line_origin,
                    line_height,
                    wrap_width,
                    selection_color,
                );
            }

            if cursor >= range.start && cursor <= range.end {
                let cursor_position = if is_placeholder {
                    point(px(0.), px(0.))
                } else {
                    line.position_for_index(
                        cursor.saturating_sub(range.start).min(range.len()),
                        line_height,
                    )
                    .unwrap_or_else(|| point(px(0.), px(0.)))
                };
                let line_cursor_bounds = Bounds::new(
                    point(
                        bounds.left() + cursor_position.x,
                        line_origin.y + cursor_position.y,
                    ),
                    size(px(1.), line_height),
                );
                if show_cursor {
                    cursor_quad = Some(Bounds::new(
                        point(
                            line_cursor_bounds.left(),
                            line_cursor_bounds.top() + (line_height - cursor_height) / 2.,
                        ),
                        size(CURSOR_WIDTH, cursor_height),
                    ));
                }
            }

            lines.push(LayoutLine {
                range,
                y,
                visual_row,
                visual_line_count,
                line,
            });
            visual_row += visual_line_count;
        }

        PrepaintState {
            lines,
            cursor: cursor_quad,
            selections,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let visual_line_count = prepaint
            .lines
            .last()
            .map(|line| line.visual_row + line.visual_line_count)
            .unwrap_or(1);
        let scroll_offset = self.editor.read(cx).scroll_offset();
        let viewport_line_count = visual_line_count.clamp(
            ComposerEditor::MIN_VISIBLE_LINES,
            ComposerEditor::MAX_VISIBLE_LINES,
        );
        let viewport_bounds = Bounds::new(
            point(
                bounds.left() - scroll_offset.x,
                bounds.top() - scroll_offset.y,
            ),
            size(
                bounds.size.width,
                window.line_height() * viewport_line_count as f32,
            ),
        );
        let focus_handle = self.editor.read(cx).focus_handle().clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );

        window.with_content_mask(
            Some(ContentMask {
                bounds: viewport_bounds,
            }),
            |window| {
                for selection in prepaint.selections.drain(..) {
                    window.paint_quad(selection);
                }

                for line in &prepaint.lines {
                    line.line
                        .paint(
                            point(bounds.left(), bounds.top() + line.y),
                            window.line_height(),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        )
                        .ok();
                }

                if let Some(cursor) = prepaint.cursor.take() {
                    window.paint_quad(fill(window.pixel_snap_bounds(cursor), cx.theme().caret));
                }
            },
        );

        let lines = std::mem::take(&mut prepaint.lines);
        self.editor.update(cx, |editor, _cx| {
            editor.set_layout(
                LayoutCache {
                    bounds,
                    viewport_bounds,
                    line_height: window.line_height(),
                    visual_line_count,
                    lines,
                },
                _cx,
            );
        });
    }
}

fn push_selection_quads(
    selections: &mut Vec<PaintQuad>,
    selection: std::ops::Range<usize>,
    line_range: std::ops::Range<usize>,
    line: &WrappedLine,
    line_origin: Point<Pixels>,
    line_height: Pixels,
    wrap_width: Pixels,
    color: Hsla,
) {
    let selection_start = selection.start.max(line_range.start);
    let selection_end = selection.end.min(line_range.end);
    if selection_start >= selection_end {
        return;
    }

    for visual_range in visual_ranges(line) {
        let visual_start = line_range.start + visual_range.range.start;
        let visual_end = line_range.start + visual_range.range.end;
        let overlap_start = selection_start.max(visual_start);
        let overlap_end = selection_end.min(visual_end);
        if overlap_start >= overlap_end {
            continue;
        }

        let start_x = if overlap_start == visual_start {
            px(0.)
        } else {
            line.position_for_index(overlap_start - line_range.start, line_height)
                .map(|position| position.x)
                .unwrap_or(px(0.))
        };
        let end_x = line
            .position_for_index(overlap_end - line_range.start, line_height)
            .map(|position| position.x)
            .unwrap_or(wrap_width);
        let y = line_height * visual_range.row as f32;
        selections.push(fill(
            Bounds::from_corners(
                point(line_origin.x + start_x, line_origin.y + y),
                point(line_origin.x + end_x, line_origin.y + y + line_height),
            ),
            color,
        ));
    }
}

struct VisualRange {
    row: usize,
    range: std::ops::Range<usize>,
}

fn visual_ranges(line: &WrappedLine) -> Vec<VisualRange> {
    let mut ranges = Vec::with_capacity(line.wrap_boundaries().len() + 1);
    let mut start = 0;
    for (row, boundary) in line.wrap_boundaries().iter().enumerate() {
        let run = &line.unwrapped_layout.runs[boundary.run_ix];
        let glyph = &run.glyphs[boundary.glyph_ix];
        let end = glyph.index;
        if start < end {
            ranges.push(VisualRange {
                row,
                range: start..end,
            });
        }
        start = end;
    }
    if start <= line.len() {
        ranges.push(VisualRange {
            row: line.wrap_boundaries().len(),
            range: start..line.len(),
        });
    }
    ranges
}

fn line_runs(
    text: &str,
    line_range: std::ops::Range<usize>,
    tokens: &[ComposerToken],
    marked_range: Option<std::ops::Range<usize>>,
    base_color: Hsla,
    token_color: Hsla,
) -> Vec<TextRun> {
    let mut boundaries = vec![line_range.start, line_range.end];
    for token in tokens {
        if ranges_overlap(&line_range, &token.range) {
            boundaries.push(token.range.start.max(line_range.start));
            boundaries.push(token.range.end.min(line_range.end));
        }
    }
    if let Some(marked_range) = &marked_range
        && ranges_overlap(&line_range, marked_range)
    {
        boundaries.push(marked_range.start.max(line_range.start));
        boundaries.push(marked_range.end.min(line_range.end));
    }

    boundaries.sort_unstable();
    boundaries.dedup();

    boundaries
        .windows(2)
        .filter_map(|window| {
            let start = window[0];
            let end = window[1];
            if start == end {
                return None;
            }
            let is_token = tokens
                .iter()
                .any(|token| start >= token.range.start && end <= token.range.end);
            let is_marked = marked_range
                .as_ref()
                .is_some_and(|range| start >= range.start && end <= range.end);
            let underline = is_marked.then_some(UnderlineStyle {
                thickness: px(1.),
                color: Some(if is_token { token_color } else { base_color }),
                wavy: false,
            });
            Some(text_run(
                text[start..end].len(),
                if is_token { token_color } else { base_color },
                underline,
            ))
        })
        .collect()
}

fn ranges_overlap(a: &std::ops::Range<usize>, b: &std::ops::Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

fn text_run(len: usize, color: Hsla, underline: Option<UnderlineStyle>) -> TextRun {
    TextRun {
        len,
        font: gpui::Font::default(),
        color,
        background_color: None,
        underline,
        strikethrough: None,
    }
}
