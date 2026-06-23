use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, ElementInputHandler,
    Entity, GlobalElementId, Hsla, InteractiveElement as _, IntoElement, LayoutId, PaintQuad,
    ParentElement as _, Pixels, Point, ShapedLine, Size, Style, Styled as _, TextAlign, TextRun,
    UnderlineStyle, Window, fill, point, px, relative, size,
};
use gpui_component::{ActiveTheme, Icon, Sizable, h_flex};

use crate::foundation::assets::IconName;

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
    visual_lines: Vec<VisualLineLayout>,
}

impl LayoutLine {
    #[cfg(test)]
    pub(super) fn first_visual_width(&self) -> Option<Pixels> {
        self.visual_lines.first().map(VisualLineLayout::width)
    }
}

#[derive(Clone)]
struct VisualLineLayout {
    range: std::ops::Range<usize>,
    y: Pixels,
    fragments: Vec<LayoutFragment>,
}

#[derive(Clone)]
enum LayoutFragment {
    Text {
        range: std::ops::Range<usize>,
        line: ShapedLine,
    },
    Token {
        range: std::ops::Range<usize>,
        size: Size<Pixels>,
    },
}

#[cfg(test)]
impl LayoutFragment {
    fn width(&self) -> Pixels {
        match self {
            Self::Text { line, .. } => line.width(),
            Self::Token { size, .. } => size.width,
        }
    }
}

impl VisualLineLayout {
    #[cfg(test)]
    fn width(&self) -> Pixels {
        self.fragments.iter().map(LayoutFragment::width).sum()
    }

    fn x_for_offset(&self, offset: usize) -> Pixels {
        let mut x = px(0.);
        for fragment in &self.fragments {
            match fragment {
                LayoutFragment::Text { range, line } => {
                    if offset <= range.end {
                        let local = offset.saturating_sub(range.start).min(line.len());
                        return x + line.x_for_index(local);
                    }
                    x += line.width();
                }
                LayoutFragment::Token { range, size } => {
                    if offset <= range.start {
                        return x;
                    }
                    if offset < range.end {
                        return x + if offset - range.start < range.end - offset {
                            px(0.)
                        } else {
                            size.width
                        };
                    }
                    if offset == range.end {
                        return x + size.width;
                    }
                    x += size.width;
                }
            }
        }
        x
    }

    fn offset_for_x(&self, x: Pixels) -> usize {
        let mut fragment_x = px(0.);
        for fragment in &self.fragments {
            match fragment {
                LayoutFragment::Text { range, line } => {
                    let end_x = fragment_x + line.width();
                    if x <= end_x {
                        let local = line.closest_index_for_x((x - fragment_x).max(px(0.)));
                        return range.start + local.min(range.len());
                    }
                    fragment_x = end_x;
                }
                LayoutFragment::Token { range, size } => {
                    let end_x = fragment_x + size.width;
                    if x <= end_x {
                        let midpoint = fragment_x + size.width / 2.;
                        return if x < midpoint { range.start } else { range.end };
                    }
                    fragment_x = end_x;
                }
            }
        }
        self.range.end
    }

    fn token_for_x(&self, x: Pixels) -> Option<ComposerTokenHit> {
        let mut fragment_x = px(0.);
        for fragment in &self.fragments {
            match fragment {
                LayoutFragment::Text { line, .. } => {
                    fragment_x += line.width();
                }
                LayoutFragment::Token { range, size } => {
                    let end_x = fragment_x + size.width;
                    if x >= fragment_x && x <= end_x {
                        return Some(ComposerTokenHit {
                            range: range.clone(),
                        });
                    }
                    fragment_x = end_x;
                }
            }
        }
        None
    }
}

#[derive(Clone)]
pub(super) struct ComposerTokenHit {
    pub(super) range: std::ops::Range<usize>,
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
        let visual = self.visual_line_for_offset(offset)?;
        let x = visual.x_for_offset(offset);

        Some(Bounds::new(
            point(self.bounds.left() + x, self.bounds.top() + visual.y),
            size(px(1.), self.line_height),
        ))
    }

    pub(super) fn offset_for_position(&self, text: &str, position: Point<Pixels>) -> usize {
        let Some(visual) = self.visual_line_for_position(position) else {
            return 0;
        };
        buffer::clamp_offset(
            text,
            visual.offset_for_x((position.x - self.bounds.left()).max(px(0.))),
        )
    }

    pub(super) fn token_hit_for_position(
        &self,
        text: &str,
        position: Point<Pixels>,
    ) -> Option<ComposerTokenHit> {
        let visual = self.visual_line_for_position(position)?;
        let hit = visual.token_for_x((position.x - self.bounds.left()).max(px(0.)))?;
        let offset = buffer::clamp_offset(text, hit.range.start);
        (offset == hit.range.start).then_some(hit)
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
        let current_ix = self.visual_line_index_for_offset(offset)?;
        let current = self.visual_line_at(current_ix)?;
        let preferred_x = preferred_x.unwrap_or_else(|| current.x_for_offset(offset));
        let target_ix = if delta < 0 {
            current_ix.saturating_sub(delta.unsigned_abs())
        } else {
            (current_ix + delta as usize).min(self.visual_line_count.saturating_sub(1))
        };
        let target = self.visual_line_at(target_ix)?;

        Some((target.offset_for_x(preferred_x), preferred_x))
    }

    fn visual_line_for_offset(&self, offset: usize) -> Option<&VisualLineLayout> {
        self.lines
            .iter()
            .flat_map(|line| &line.visual_lines)
            .find(|line| offset >= line.range.start && offset <= line.range.end)
            .or_else(|| self.lines.last().and_then(|line| line.visual_lines.last()))
    }

    fn visual_line_index_for_offset(&self, offset: usize) -> Option<usize> {
        self.lines
            .iter()
            .flat_map(|line| &line.visual_lines)
            .position(|line| offset >= line.range.start && offset <= line.range.end)
            .or_else(|| self.visual_line_count.checked_sub(1))
    }

    fn visual_line_for_position(&self, position: Point<Pixels>) -> Option<&VisualLineLayout> {
        let y = (position.y - self.bounds.top()).max(px(0.));
        self.lines
            .iter()
            .flat_map(|line| &line.visual_lines)
            .find(|line| y >= line.y && y < line.y + self.line_height)
            .or_else(|| self.lines.last().and_then(|line| line.visual_lines.last()))
    }

    fn visual_line_at(&self, ix: usize) -> Option<&VisualLineLayout> {
        self.lines
            .iter()
            .flat_map(|line| &line.visual_lines)
            .nth(ix)
    }
}

struct PaintLine {
    visual_row: usize,
    visual_lines: Vec<PaintVisualLine>,
}

struct PaintVisualLine {
    range: std::ops::Range<usize>,
    y: Pixels,
    width: Pixels,
    fragments: Vec<PaintFragment>,
}

enum PaintFragment {
    Text {
        range: std::ops::Range<usize>,
        line: ShapedLine,
    },
    Token {
        range: std::ops::Range<usize>,
        size: Size<Pixels>,
        element: Option<AnyElement>,
    },
}

impl PaintFragment {
    fn width(&self) -> Pixels {
        match self {
            Self::Text { line, .. } => line.width(),
            Self::Token { size, .. } => size.width,
        }
    }

    fn layout_fragment(&self) -> LayoutFragment {
        match self {
            Self::Text { range, line } => LayoutFragment::Text {
                range: range.clone(),
                line: line.clone(),
            },
            Self::Token { range, size, .. } => LayoutFragment::Token {
                range: range.clone(),
                size: *size,
            },
        }
    }
}

impl PaintVisualLine {
    fn layout_line(&self) -> VisualLineLayout {
        VisualLineLayout {
            range: self.range.clone(),
            y: self.y,
            fragments: self
                .fragments
                .iter()
                .map(PaintFragment::layout_fragment)
                .collect(),
        }
    }
}

impl PaintLine {
    fn layout_line(&self) -> LayoutLine {
        LayoutLine {
            visual_lines: self
                .visual_lines
                .iter()
                .map(PaintVisualLine::layout_line)
                .collect(),
        }
    }
}

pub(super) struct PrepaintState {
    lines: Vec<PaintLine>,
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
        let selection_color = cx.theme().blue.opacity(0.22);
        let wrap_width = bounds.size.width.max(px(1.));
        let _ = editor;

        let mut lines = if text.is_empty() {
            vec![layout_placeholder_line(
                placeholder.to_string(),
                font_size,
                placeholder_color,
                wrap_width,
                window,
            )]
        } else {
            buffer::line_ranges(&text)
                .into_iter()
                .scan(0usize, |visual_row, range| {
                    let y = line_height * *visual_row as f32;
                    let line = layout_editor_line(LayoutEditorLineInput {
                        text: &text,
                        range,
                        y,
                        visual_row: *visual_row,
                        selection: selection.clone(),
                        marked_range: marked_range.clone(),
                        tokens: &tokens,
                        font_size,
                        line_height,
                        wrap_width,
                        base_color,
                        window,
                        cx,
                    });
                    *visual_row += line.visual_lines.len().max(1);
                    Some(line)
                })
                .collect::<Vec<_>>()
        };

        let mut selections = Vec::new();
        if !selection.is_empty() && !text.is_empty() {
            for line in &lines {
                push_selection_quads(
                    &mut selections,
                    SelectionQuadInput {
                        selection: selection.clone(),
                        line,
                        origin: bounds.origin,
                        line_height,
                        color: selection_color,
                    },
                );
            }
        }

        let cursor_quad = (!text.is_empty() || show_cursor)
            .then(|| {
                cursor_bounds_for_lines(&lines, cursor, bounds.origin, line_height).map(|bounds| {
                    Bounds::new(
                        point(
                            bounds.left(),
                            bounds.top() + (line_height - cursor_height) / 2.,
                        ),
                        size(CURSOR_WIDTH, cursor_height),
                    )
                })
            })
            .flatten();

        prepaint_token_elements(&mut lines, bounds.origin, line_height, window, cx);

        PrepaintState {
            lines,
            cursor: show_cursor.then_some(cursor_quad).flatten(),
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
            .map(|line| line.visual_row + line.visual_lines.len().max(1))
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

                for line in &mut prepaint.lines {
                    for visual in &mut line.visual_lines {
                        let mut fragment_origin = point(bounds.left(), bounds.top() + visual.y);
                        for fragment in &mut visual.fragments {
                            match fragment {
                                PaintFragment::Text { line, .. } => {
                                    line.paint(
                                        fragment_origin,
                                        window.line_height(),
                                        TextAlign::Left,
                                        None,
                                        window,
                                        cx,
                                    )
                                    .ok();
                                    fragment_origin.x += line.width();
                                }
                                PaintFragment::Token { element, size, .. } => {
                                    if let Some(element) = element {
                                        element.paint(window, cx);
                                    }
                                    fragment_origin.x += size.width;
                                }
                            }
                        }
                    }
                }

                if let Some(cursor) = prepaint.cursor.take() {
                    window.paint_quad(fill(window.pixel_snap_bounds(cursor), cx.theme().caret));
                }
            },
        );

        let lines = prepaint
            .lines
            .iter()
            .map(PaintLine::layout_line)
            .collect::<Vec<_>>();
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

struct LayoutEditorLineInput<'a> {
    text: &'a str,
    range: std::ops::Range<usize>,
    y: Pixels,
    visual_row: usize,
    selection: std::ops::Range<usize>,
    marked_range: Option<std::ops::Range<usize>>,
    tokens: &'a [ComposerToken],
    font_size: Pixels,
    line_height: Pixels,
    wrap_width: Pixels,
    base_color: Hsla,
    window: &'a mut Window,
    cx: &'a mut App,
}

fn layout_editor_line(input: LayoutEditorLineInput<'_>) -> PaintLine {
    if input.range.is_empty() {
        return PaintLine {
            visual_row: input.visual_row,
            visual_lines: vec![PaintVisualLine {
                range: input.range,
                y: input.y,
                width: px(0.),
                fragments: Vec::new(),
            }],
        };
    }

    let fragments = source_fragments(
        input.text,
        input.range.clone(),
        input.tokens,
        input.line_height,
        input.window,
        input.cx,
    );
    let wrap_fragments = fragments
        .iter()
        .map(|fragment| match fragment {
            SourceFragment::Text { range } => gpui::LineFragment::text(&input.text[range.clone()]),
            SourceFragment::Token { range, size, .. } => {
                gpui::LineFragment::element(size.width, range.len())
            }
        })
        .collect::<Vec<_>>();
    let mut wrapper = input
        .window
        .text_system()
        .line_wrapper(input.window.text_style().font(), input.font_size);
    let mut local_ranges = Vec::new();
    let mut start = 0;
    for boundary in wrapper.wrap_line(&wrap_fragments, input.wrap_width) {
        if boundary.ix > start {
            local_ranges.push(start..boundary.ix);
        }
        start = boundary.ix;
    }
    if start <= input.range.len() {
        local_ranges.push(start..input.range.len());
    }
    if local_ranges.is_empty() {
        local_ranges.push(0..0);
    }

    let visual_lines = local_ranges
        .into_iter()
        .enumerate()
        .map(|(ix, local)| {
            let absolute = input.range.start + local.start..input.range.start + local.end;
            let fragments = paint_fragments_for_range(
                input.text,
                &fragments,
                absolute.clone(),
                input.selection.clone(),
                input.marked_range.clone(),
                input.font_size,
                input.line_height,
                input.base_color,
                input.window,
                input.cx,
            );
            let width = fragments.iter().map(PaintFragment::width).sum();
            PaintVisualLine {
                range: absolute,
                y: input.y + input.line_height * ix as f32,
                width,
                fragments,
            }
        })
        .collect();

    PaintLine {
        visual_row: input.visual_row,
        visual_lines,
    }
}

fn layout_placeholder_line(
    placeholder: String,
    font_size: Pixels,
    color: Hsla,
    wrap_width: Pixels,
    window: &mut Window,
) -> PaintLine {
    let line = window
        .text_system()
        .shape_line(
            placeholder.clone().into(),
            font_size,
            &[text_run(placeholder.len(), color, None)],
            Some(wrap_width),
        )
        .with_len(0);
    let width = line.width();

    PaintLine {
        visual_row: 0,
        visual_lines: vec![PaintVisualLine {
            range: 0..0,
            y: px(0.),
            width,
            fragments: vec![PaintFragment::Text { range: 0..0, line }],
        }],
    }
}

#[derive(Clone)]
enum SourceFragment {
    Text {
        range: std::ops::Range<usize>,
    },
    Token {
        token: ComposerToken,
        range: std::ops::Range<usize>,
        size: Size<Pixels>,
    },
}

fn source_fragments(
    text: &str,
    line_range: std::ops::Range<usize>,
    tokens: &[ComposerToken],
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> Vec<SourceFragment> {
    let mut fragments = Vec::new();
    let mut cursor = line_range.start;

    for token in tokens {
        if token.range.end <= line_range.start {
            continue;
        }
        if token.range.start >= line_range.end {
            break;
        }
        if token.range.start < line_range.start || token.range.end > line_range.end {
            continue;
        }
        if cursor < token.range.start {
            fragments.push(SourceFragment::Text {
                range: cursor..token.range.start,
            });
        }
        fragments.push(SourceFragment::Token {
            token: token.clone(),
            range: token.range.clone(),
            size: measure_token_chip(token, line_height, window, cx),
        });
        cursor = token.range.end;
    }

    if cursor < line_range.end {
        fragments.push(SourceFragment::Text {
            range: cursor..line_range.end,
        });
    }

    if fragments.is_empty() && !text[line_range.clone()].is_empty() {
        fragments.push(SourceFragment::Text { range: line_range });
    }

    fragments
}

struct PaintFragmentInput<'a> {
    text: &'a str,
    source: &'a SourceFragment,
    range: std::ops::Range<usize>,
    selection: std::ops::Range<usize>,
    marked_range: Option<std::ops::Range<usize>>,
    font_size: Pixels,
    line_height: Pixels,
    base_color: Hsla,
    window: &'a mut Window,
    cx: &'a mut App,
}

fn paint_fragments_for_range(
    text: &str,
    source_fragments: &[SourceFragment],
    visual_range: std::ops::Range<usize>,
    selection: std::ops::Range<usize>,
    marked_range: Option<std::ops::Range<usize>>,
    font_size: Pixels,
    line_height: Pixels,
    base_color: Hsla,
    window: &mut Window,
    cx: &mut App,
) -> Vec<PaintFragment> {
    source_fragments
        .iter()
        .filter_map(|source| {
            let source_range = match source {
                SourceFragment::Text { range } | SourceFragment::Token { range, .. } => range,
            };
            let overlap = overlap_range(source_range, &visual_range)?;
            paint_fragment(PaintFragmentInput {
                text,
                source,
                range: overlap,
                selection: selection.clone(),
                marked_range: marked_range.clone(),
                font_size,
                line_height,
                base_color,
                window,
                cx,
            })
        })
        .collect()
}

fn paint_fragment(input: PaintFragmentInput<'_>) -> Option<PaintFragment> {
    match input.source {
        SourceFragment::Text { .. } => {
            if input.range.is_empty() {
                return None;
            }
            let runs = text_runs_for_range(
                input.text,
                input.range.clone(),
                input.marked_range.clone(),
                input.base_color,
            );
            let line = input.window.text_system().shape_line(
                input.text[input.range.clone()].to_string().into(),
                input.font_size,
                &runs,
                None,
            );
            Some(PaintFragment::Text {
                range: input.range,
                line,
            })
        }
        SourceFragment::Token {
            token,
            range,
            size: token_size,
        } => {
            if input.range.start != range.start || input.range.end != range.end {
                return None;
            }
            let selected = ranges_overlap(&input.selection, range);
            let mut element = token_chip(token, selected, input.line_height, input.cx);
            let measured = element.layout_as_root(
                size(
                    AvailableSpace::Definite(token_size.width),
                    AvailableSpace::Definite(input.line_height),
                ),
                input.window,
                input.cx,
            );
            Some(PaintFragment::Token {
                range: range.clone(),
                size: measured,
                element: Some(element),
            })
        }
    }
}

fn prepaint_token_elements(
    lines: &mut [PaintLine],
    origin: Point<Pixels>,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    for line in lines {
        for visual in &mut line.visual_lines {
            let mut fragment_origin = point(origin.x, origin.y + visual.y);
            for fragment in &mut visual.fragments {
                match fragment {
                    PaintFragment::Text { line, .. } => {
                        fragment_origin.x += line.width();
                    }
                    PaintFragment::Token { element, size, .. } => {
                        if let Some(element) = element {
                            let element_origin = point(
                                fragment_origin.x,
                                fragment_origin.y + (line_height - size.height) / 2.,
                            );
                            element.prepaint_at(element_origin, window, cx);
                        }
                        fragment_origin.x += size.width;
                    }
                }
            }
        }
    }
}

fn cursor_bounds_for_lines(
    lines: &[PaintLine],
    cursor: usize,
    origin: Point<Pixels>,
    line_height: Pixels,
) -> Option<Bounds<Pixels>> {
    for line in lines {
        for visual in &line.visual_lines {
            if cursor >= visual.range.start && cursor <= visual.range.end {
                return Some(Bounds::new(
                    point(
                        origin.x + visual.layout_line().x_for_offset(cursor),
                        origin.y + visual.y,
                    ),
                    size(px(1.), line_height),
                ));
            }
        }
    }

    lines.last().and_then(|line| {
        line.visual_lines.last().map(|visual| {
            Bounds::new(
                point(origin.x + visual.width, origin.y + visual.y),
                size(px(1.), line_height),
            )
        })
    })
}

struct SelectionQuadInput<'a> {
    selection: std::ops::Range<usize>,
    line: &'a PaintLine,
    origin: Point<Pixels>,
    line_height: Pixels,
    color: Hsla,
}

fn push_selection_quads(selections: &mut Vec<PaintQuad>, input: SelectionQuadInput<'_>) {
    for visual in &input.line.visual_lines {
        let selection_start = input.selection.start.max(visual.range.start);
        let selection_end = input.selection.end.min(visual.range.end);
        if selection_start >= selection_end {
            continue;
        }

        let layout = visual.layout_line();
        let start_x = layout.x_for_offset(selection_start);
        let end_x = layout.x_for_offset(selection_end);
        selections.push(fill(
            Bounds::from_corners(
                point(input.origin.x + start_x, input.origin.y + visual.y),
                point(
                    input.origin.x + end_x,
                    input.origin.y + visual.y + input.line_height,
                ),
            ),
            input.color,
        ));
    }
}

fn text_runs_for_range(
    text: &str,
    range: std::ops::Range<usize>,
    marked_range: Option<std::ops::Range<usize>>,
    base_color: Hsla,
) -> Vec<TextRun> {
    let mut boundaries = vec![range.start, range.end];
    if let Some(marked_range) = &marked_range
        && ranges_overlap(&range, marked_range)
    {
        boundaries.push(marked_range.start.max(range.start));
        boundaries.push(marked_range.end.min(range.end));
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
            let is_marked = marked_range
                .as_ref()
                .is_some_and(|range| start >= range.start && end <= range.end);
            let underline = is_marked.then_some(UnderlineStyle {
                thickness: px(1.),
                color: Some(base_color),
                wavy: false,
            });
            Some(text_run(text[start..end].len(), base_color, underline))
        })
        .collect()
}

fn measure_token_chip(
    token: &ComposerToken,
    line_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> Size<Pixels> {
    let mut element = token_chip(token, false, line_height, cx);
    element.layout_as_root(
        size(
            AvailableSpace::MinContent,
            AvailableSpace::Definite(line_height),
        ),
        window,
        cx,
    )
}

fn token_chip(
    token: &ComposerToken,
    selected: bool,
    line_height: Pixels,
    cx: &mut App,
) -> AnyElement {
    let height = (line_height - px(2.)).max(px(18.));
    h_flex()
        .id(("ai-chat2-skill-token", token.id))
        .h(height)
        .items_center()
        .gap_1()
        .px_2()
        .rounded(px(5.))
        .border_1()
        .border_color(if selected {
            cx.theme().blue
        } else {
            cx.theme().border
        })
        .bg(if selected {
            cx.theme().blue.opacity(0.14)
        } else {
            cx.theme().muted
        })
        .child(
            Icon::new(IconName::Sparkles)
                .with_size(px(13.))
                .text_color(cx.theme().blue),
        )
        .child(
            gpui::div()
                .text_color(cx.theme().foreground)
                .child(token.name.clone()),
        )
        .into_any_element()
}

fn overlap_range(
    left: &std::ops::Range<usize>,
    right: &std::ops::Range<usize>,
) -> Option<std::ops::Range<usize>> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    (start < end).then_some(start..end)
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
