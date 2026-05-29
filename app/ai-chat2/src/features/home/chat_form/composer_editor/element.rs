use gpui::{
    App, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity, GlobalElementId,
    Hsla, IntoElement, LayoutId, PaintQuad, Pixels, ShapedLine, Style, TextAlign, TextRun,
    UnderlineStyle, Window, fill, point, px, relative, size,
};
use gpui_component::ActiveTheme;

use super::{ComposerEditor, buffer, token::ComposerToken};

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
    pub(super) line: ShapedLine,
}

pub(super) struct LayoutCache {
    pub(super) bounds: Bounds<Pixels>,
    pub(super) line_height: Pixels,
    pub(super) lines: Vec<LayoutLine>,
}

pub(super) struct PrepaintState {
    lines: Vec<LayoutLine>,
    cursor: Option<PaintQuad>,
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
        let line_count = self
            .editor
            .read(cx)
            .display_line_count()
            .clamp(2, ComposerEditor::MAX_VISIBLE_LINES);
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
        let focused = editor.focus_handle().is_focused(window);
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let base_color = text_style.color;
        let placeholder_color = cx.theme().muted_foreground.opacity(0.72);
        let token_color = cx.theme().blue;
        let selection_color = cx.theme().blue.opacity(0.22);
        let caret_color = cx.theme().blue;

        let ranges = buffer::line_ranges(&text);
        let is_placeholder = text.is_empty();
        let mut lines = Vec::with_capacity(ranges.len());
        let mut selections = Vec::new();
        let mut cursor_quad = None;

        for (line_ix, range) in ranges.into_iter().enumerate() {
            let y = line_height * line_ix as f32;
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

            let line = window
                .text_system()
                .shape_line(line_text.into(), font_size, &runs, None);
            let line_origin = point(bounds.left(), bounds.top() + y);

            if !selection.is_empty() && !is_placeholder {
                let overlap_start = selection.start.max(range.start);
                let overlap_end = selection.end.min(range.end);
                if overlap_start < overlap_end {
                    selections.push(fill(
                        Bounds::from_corners(
                            point(
                                bounds.left() + line.x_for_index(overlap_start - range.start),
                                line_origin.y,
                            ),
                            point(
                                bounds.left() + line.x_for_index(overlap_end - range.start),
                                line_origin.y + line_height,
                            ),
                        ),
                        selection_color,
                    ));
                }
            }

            if focused && cursor >= range.start && cursor <= range.end {
                let cursor_x = if is_placeholder {
                    px(0.)
                } else {
                    line.x_for_index(cursor.saturating_sub(range.start))
                };
                cursor_quad = Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_x, line_origin.y),
                        size(px(1.5), line_height),
                    ),
                    caret_color,
                ));
            }

            lines.push(LayoutLine { range, y, line });
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
        let focus_handle = self.editor.read(cx).focus_handle().clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
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
                window.paint_quad(cursor);
            }
        });

        let lines = std::mem::take(&mut prepaint.lines);
        self.editor.update(cx, |editor, _cx| {
            editor.last_layout = Some(LayoutCache {
                bounds,
                line_height: window.line_height(),
                lines,
            });
        });
    }
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
