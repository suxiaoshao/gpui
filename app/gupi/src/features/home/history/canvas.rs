mod geometry;
use super::*;
use crate::foundation::i18n::t_with_args;
use crate::state::history::canvas::{Tree, X_GAP};
use fluent_bundle::FluentArgs;
use geometry::{
    Camera, clip_segment, curve, disclosure_bounds, distance, label_bounds, path_distance,
};

pub(crate) enum CanvasEvent {
    Preview(String),
    Fork(String),
}
pub(crate) struct HistoryCanvas {
    tree: Tree,
    camera: Camera,
    bounds: Bounds<Pixels>,
    initialized: bool,
    focus: FocusHandle,
    hover: Option<Hit>,
    tooltip: Option<HoverTip>,
    pointer: Point<f32>,
    drag: Option<Drag>,
    space: bool,
    labels: Vec<(Bounds<f32>, usize)>,
    collapse_controls: Vec<(Bounds<f32>, usize)>,
    forkable: HashSet<String>,
    can_fork: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    Node(usize),
    Edge(usize),
    Collapse(usize),
}
enum HoverTip {
    Waiting { target: Hit, _task: Task<()> },
    Visible { target: Hit, tooltip: AnyTooltip },
}
impl HoverTip {
    fn target(&self) -> Hit {
        match self {
            Self::Waiting { target, .. } | Self::Visible { target, .. } => *target,
        }
    }
}
struct Drag {
    start: Point<f32>,
    pan: Point<f32>,
    moved: bool,
    target: Option<Hit>,
}
struct Frame {
    elements: Vec<AnyElement>,
    lines: Vec<(Vec<Point<f32>>, bool, Hsla)>,
    mask: ContentMask<Pixels>,
}
impl EventEmitter<CanvasEvent> for HistoryCanvas {}
impl HistoryCanvas {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            tree: Tree::default(),
            camera: Camera::default(),
            bounds: Bounds::default(),
            initialized: false,
            focus: cx.focus_handle(),
            hover: None,
            tooltip: None,
            pointer: point(0., 0.),
            drag: None,
            space: false,
            labels: vec![],
            collapse_controls: vec![],
            forkable: HashSet::new(),
            can_fork: false,
        }
    }
    pub fn sync(
        &mut self,
        rows: Option<(Vec<HistoryRow>, HashSet<String>)>,
        preview: Option<String>,
        forkable: HashSet<String>,
        can_fork: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some((rows, entries)) = rows {
            self.tooltip = None;
            let anchor = self.anchor(None);
            self.tree.retain_entries(&entries);
            self.tree.replace(rows, preview);
            self.restore(anchor);
            self.hover = None;
            self.drag = None;
        } else if self.tree.preview != preview {
            self.tooltip = None;
            let anchor = self.anchor(None);
            self.tree.set_preview(preview);
            self.restore(anchor);
            self.hover = None;
        }
        self.forkable = forkable;
        self.can_fork = can_fork;
        cx.notify();
    }
    pub fn clear_pointer(&mut self, cx: &mut Context<Self>) {
        self.tooltip = None;
        self.hover = None;
        self.drag = None;
        self.space = false;
        cx.notify();
    }
    pub fn reveal(&mut self, id: String, cx: &mut Context<Self>) {
        self.tooltip = None;
        let anchor = self.anchor(None);
        self.tree.select(id);
        self.restore(anchor);
        self.hover = None;
        cx.notify();
    }
    fn anchor(&self, node: Option<usize>) -> Option<(String, Point<f32>)> {
        let inside = |p: Point<f32>| {
            p.x >= 0.
                && p.y >= 0.
                && p.x <= f32::from(self.bounds.size.width)
                && p.y <= f32::from(self.bounds.size.height)
        };
        let node = node
            .or_else(|| {
                self.tree
                    .selected_node()
                    .filter(|&i| inside(self.position(i)))
            })
            .or_else(|| {
                self.visible()
                    .0
                    .into_iter()
                    .filter(|&i| inside(self.position(i)))
                    .min_by(|&a, &b| {
                        distance(self.position(a), self.center())
                            .total_cmp(&distance(self.position(b), self.center()))
                    })
            })?;
        Some((
            self.tree.rows[self.tree.nodes[node].row].id.clone(),
            self.position(node),
        ))
    }
    fn restore(&mut self, anchor: Option<(String, Point<f32>)>) {
        if let Some((id, screen)) = anchor
            && let Some(i) = self.tree.node_for(&id)
        {
            let n = &self.tree.nodes[i];
            self.camera.pan = screen - point(n.x, n.y) * self.camera.zoom;
        }
    }
    fn center(&self) -> Point<f32> {
        point(
            f32::from(self.bounds.size.width) / 2.,
            f32::from(self.bounds.size.height) / 2.,
        )
    }
    fn position(&self, node: usize) -> Point<f32> {
        let n = &self.tree.nodes[node];
        self.camera.screen(point(n.x, n.y))
    }
    fn local(&self, point: Point<Pixels>) -> Point<f32> {
        (point - self.bounds.origin).map(f32::from)
    }
    fn visible(&self) -> (Vec<usize>, Vec<usize>) {
        let top = self.camera.world(point(-180., -80.));
        let bottom = self.camera.world(point(
            f32::from(self.bounds.size.width) + 180.,
            f32::from(self.bounds.size.height) + 80.,
        ));
        self.tree.visible(top.x, top.y, bottom.x, bottom.y)
    }
    fn edge_curve(&self, edge: usize) -> [Point<f32>; 33] {
        let e = &self.tree.edges[edge];
        let radius = (10. * self.camera.zoom).clamp(3., 22.);
        curve(
            self.position(e.from) + point(0., radius),
            self.position(e.to) - point(0., radius),
        )
    }
    fn hit(&self, p: Point<f32>) -> Option<Hit> {
        if let Some((_, segment)) = self
            .collapse_controls
            .iter()
            .find(|(bounds, _)| bounds.contains(&p))
        {
            return Some(Hit::Collapse(*segment));
        }
        let (nodes, edges) = self.visible();
        let radius = (12. * self.camera.zoom).max(12.);
        if let Some(i) = nodes
            .into_iter()
            .filter(|&i| distance(p, self.position(i)) <= radius)
            .min_by(|&a, &b| {
                distance(p, self.position(a)).total_cmp(&distance(p, self.position(b)))
            })
        {
            return Some(Hit::Node(i));
        }
        if let Some((_, i)) = self.labels.iter().find(|(bounds, _)| bounds.contains(&p)) {
            return Some(Hit::Node(*i));
        }
        edges
            .into_iter()
            .filter(|&i| !self.tree.edges[i].hidden.is_empty())
            .map(|i| {
                let curve = self.edge_curve(i);
                let number = disclosure_bounds(curve[16], self.tree.edges[i].hidden.len());
                (
                    i,
                    if number.contains(&p) {
                        0.
                    } else {
                        path_distance(p, &curve)
                    },
                )
            })
            .filter(|(_, d)| *d <= 12.)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| Hit::Edge(i))
    }
    fn zoom(&mut self, factor: f32, anchor: Point<f32>, cx: &mut Context<Self>) {
        self.tooltip = None;
        self.camera.zoom_at(anchor, self.camera.zoom * factor);
        self.hover = None;
        self.drag = None;
        self.labels.clear();
        self.collapse_controls.clear();
        cx.notify();
    }
    fn locate(&mut self, cx: &mut Context<Self>) {
        self.tooltip = None;
        let id = self
            .tree
            .current
            .clone()
            .or_else(|| self.tree.selected.clone());
        if let Some(id) = id {
            self.tree.select(id.clone());
            if let Some(i) = self.tree.node_for(&id) {
                self.camera.zoom = self.camera.zoom.max(1.);
                let n = &self.tree.nodes[i];
                self.camera.pan = self.center() - point(n.x, n.y) * self.camera.zoom;
                self.initialized = true;
            }
        }
        self.hover = None;
        cx.notify();
    }
    fn fit(&mut self, cx: &mut Context<Self>) {
        self.tooltip = None;
        if self.tree.nodes.is_empty() {
            return;
        }
        let mut lo = point(f32::INFINITY, f32::INFINITY);
        let mut hi = point(f32::NEG_INFINITY, f32::NEG_INFINITY);
        for n in &self.tree.nodes {
            lo.x = lo.x.min(n.x);
            lo.y = lo.y.min(n.y);
            hi.x = hi.x.max(n.x);
            hi.y = hi.y.max(n.y);
        }
        let w = (f32::from(self.bounds.size.width) - 56.).max(1.);
        let h = (f32::from(self.bounds.size.height) - 64.).max(1.);
        self.camera.zoom = (w / (hi.x - lo.x).max(X_GAP))
            .min(h / (hi.y - lo.y).max(64.))
            .clamp(0.001, 1.5);
        self.camera.pan = self.center() - (lo + hi) / 2. * self.camera.zoom;
        self.initialized = true;
        self.hover = None;
        self.labels.clear();
        self.collapse_controls.clear();
        cx.notify();
    }
    fn expand(&mut self, edge: usize, cx: &mut Context<Self>) {
        self.tooltip = None;
        let anchor = self.anchor(Some(self.tree.edges[edge].from));
        self.tree.expand(edge);
        self.restore(anchor);
        self.hover = None;
        self.labels.clear();
        self.collapse_controls.clear();
        cx.notify();
    }
    fn collapse(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        self.tooltip = None;
        let Some(segment) = self
            .tree
            .segments
            .iter()
            .position(|s| self.tree.rows[s.from].id == from && self.tree.rows[s.to].id == to)
        else {
            return;
        };
        let anchor = self.anchor(self.tree.node_for(from));
        self.tree.collapse(segment);
        self.restore(anchor);
        self.hover = None;
        self.labels.clear();
        self.collapse_controls.clear();
        cx.notify();
    }
    fn choose(&mut self, node: usize, preview: bool, cx: &mut Context<Self>) {
        let id = self.tree.rows[self.tree.nodes[node].row].id.clone();
        self.tree.select(id.clone());
        if preview {
            cx.emit(CanvasEvent::Preview(id));
        }
        cx.notify();
    }
    fn keyboard(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus.contains_focused(window, cx) {
            return;
        }
        let key = event.keystroke.key.as_str();
        if key == "space" {
            self.space = true;
            cx.stop_propagation();
            return;
        }
        if matches!(key, "+" | "=" | "-") {
            self.zoom(if key == "-" { 1. / 1.2 } else { 1.2 }, self.center(), cx);
            cx.stop_propagation();
            return;
        }
        if key == "0" {
            self.fit(cx);
            cx.stop_propagation();
            return;
        }
        let Some(selected) = self.tree.selected_node() else {
            return;
        };
        let node = &self.tree.nodes[selected];
        let next = match key {
            "up" => node.parent,
            "down" => node.children.first().copied(),
            "left" | "right" => {
                let siblings = node
                    .parent
                    .map(|p| self.tree.nodes[p].children.clone())
                    .unwrap_or_else(|| {
                        self.tree
                            .nodes
                            .iter()
                            .enumerate()
                            .filter_map(|(i, n)| n.parent.is_none().then_some(i))
                            .collect()
                    });
                let index = siblings.iter().position(|&i| i == selected).unwrap();
                let index = if key == "left" {
                    index.checked_sub(1)
                } else {
                    index.checked_add(1)
                };
                index.and_then(|i| siblings.get(i)).copied()
            }
            "enter" => {
                self.choose(selected, true, cx);
                cx.stop_propagation();
                return;
            }
            "e" => {
                if let Some(edge) = self
                    .tree
                    .edges
                    .iter()
                    .position(|e| e.from == selected && !e.hidden.is_empty())
                {
                    self.expand(edge, cx);
                }
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        if let Some(next) = next {
            self.choose(next, false, cx);
            let p = self.position(next);
            let c = self.center();
            if p.x < 24. || p.y < 24. || p.x > c.x * 2. - 24. || p.y > c.y * 2. - 24. {
                self.camera.pan += c - p;
            }
        }
        cx.stop_propagation();
    }
    fn tooltip_text(&self, target: Hit, cx: &App) -> String {
        match target {
            Hit::Edge(i) => {
                count_text(cx, "history-canvas-expand", self.tree.edges[i].hidden.len())
            }
            Hit::Collapse(i) => {
                count_text(cx, "history-canvas-collapse", self.tree.collapse_count(i))
            }
            Hit::Node(i) => {
                let row = &self.tree.rows[self.tree.nodes[i].row];
                format!(
                    "{}\n{}",
                    row.title,
                    presentation::tooltip(
                        row,
                        self.tree.preview.as_deref() == Some(row.id.as_str()),
                        cx
                    )
                )
            }
        }
    }
    fn prepare_tooltip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hover = self.hover.filter(|_| self.drag.is_none());
        if self.tooltip.as_ref().map(HoverTip::target) != hover {
            self.tooltip = None;
        }
        if let Some(target) = hover
            && self.tooltip.is_none()
        {
            let owner = cx.entity().downgrade();
            let task = window.spawn(cx, async move |cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(450))
                    .await;
                let _ = cx.update(|window, cx| {
                    let check_owner = owner.clone();
                    let _ = owner.update(cx, |this, cx| {
                        if this.hover != Some(target) || this.drag.is_some() {
                            return;
                        }
                        let view = Tooltip::new(this.tooltip_text(target, cx)).build(window, cx);
                        this.tooltip = Some(HoverTip::Visible {
                            target,
                            tooltip: AnyTooltip {
                                view,
                                mouse_position: window.mouse_position(),
                                check_visible_and_update: Rc::new(move |_, window, cx| {
                                    check_owner.upgrade().is_some_and(|entity| {
                                        let this = entity.read(cx);
                                        this.hover == Some(target)
                                            && this.drag.is_none()
                                            && this.bounds.contains(&window.mouse_position())
                                    })
                                }),
                            },
                        });
                        cx.notify();
                    });
                });
            });
            self.tooltip = Some(HoverTip::Waiting {
                target,
                _task: task,
            });
        }
        if let Some(HoverTip::Visible { tooltip, .. }) = &self.tooltip {
            window.set_tooltip(tooltip.clone());
        }
    }
}
impl Render for HistoryCanvas {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let draw = cx.entity();
        let paint = draw.clone();
        let toolbar = h_flex()
            .px_1()
            .py_1()
            .gap_1()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .bg(cx.theme().background)
            .child(
                Button::new("canvas-zoom-out")
                    .ghost()
                    .xsmall()
                    .icon(IconName::ZoomOut)
                    .tooltip(t(cx, "history-canvas-zoom-out"))
                    .accessibility_label(t(cx, "history-canvas-zoom-out"))
                    .on_click(cx.listener(|this, _, _, cx| this.zoom(1. / 1.2, this.center(), cx))),
            )
            .child(
                div()
                    .min_w(px(36.))
                    .text_xs()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{:.0}%", self.camera.zoom * 100.)),
            )
            .child(
                Button::new("canvas-zoom-in")
                    .ghost()
                    .xsmall()
                    .icon(IconName::ZoomIn)
                    .tooltip(t(cx, "history-canvas-zoom-in"))
                    .accessibility_label(t(cx, "history-canvas-zoom-in"))
                    .on_click(cx.listener(|this, _, _, cx| this.zoom(1.2, this.center(), cx))),
            )
            .child(div().w(px(1.)).h_4().mx_1().bg(cx.theme().border))
            .child(
                Button::new("canvas-fit")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Scan)
                    .tooltip(t(cx, "history-canvas-fit"))
                    .accessibility_label(t(cx, "history-canvas-fit"))
                    .on_click(cx.listener(|this, _, _, cx| this.fit(cx))),
            )
            .child(
                Button::new("canvas-current")
                    .ghost()
                    .xsmall()
                    .icon(IconName::LocateFixed)
                    .tooltip(t(cx, "history-canvas-current"))
                    .accessibility_label(t(cx, "history-canvas-current"))
                    .on_click(cx.listener(|this, _, _, cx| this.locate(cx))),
            );
        v_flex()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .id("history-canvas-surface")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .track_focus(&self.focus)
                    .aria_label(t(cx, "history-canvas-help"))
                    .cursor(if self.drag.as_ref().is_some_and(|d| d.moved) {
                        CursorStyle::ClosedHand
                    } else if self.hover.is_some() {
                        CursorStyle::PointingHand
                    } else {
                        CursorStyle::OpenHand
                    })
                    .on_key_down(cx.listener(Self::keyboard))
                    .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, _| {
                        if event.keystroke.key == "space" {
                            this.space = false;
                        }
                    }))
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                        let delta = event.delta.pixel_delta(window.line_height()).map(f32::from);
                        if event.modifiers.platform || event.modifiers.control {
                            this.zoom((-delta.y * 0.005).exp(), this.local(event.position), cx);
                        } else {
                            this.camera.pan += delta;
                            this.hover = None;
                            this.drag = None;
                            this.labels.clear();
                            this.collapse_controls.clear();
                            cx.notify();
                        }
                        cx.stop_propagation();
                    }))
                    .on_pinch(cx.listener(|this, event: &PinchEvent, _, cx| {
                        this.zoom((1. + event.delta).max(0.01), this.local(event.position), cx);
                        cx.stop_propagation();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            window.focus(&this.focus, cx);
                            this.tooltip = None;
                            let p = this.local(event.position);
                            this.drag = Some(Drag {
                                start: p,
                                pan: this.camera.pan,
                                moved: false,
                                target: if this.space { None } else { this.hit(p) },
                            });
                            this.hover = None;
                            cx.notify();
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        let p = this.local(event.position);
                        this.pointer = p;
                        if let Some(drag) = &mut this.drag {
                            if event.pressed_button != Some(MouseButton::Left) {
                                this.drag = None;
                            } else if drag.moved || distance(p, drag.start) > 4. {
                                drag.moved = true;
                                this.camera.pan = drag.pan + p - drag.start;
                                this.hover = None;
                                this.labels.clear();
                                this.collapse_controls.clear();
                                cx.notify();
                                cx.stop_propagation();
                                return;
                            }
                        }
                        let hit = this.hit(p);
                        if this.hover != hit || matches!(hit, Some(Hit::Edge(_))) {
                            this.hover = hit;
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseUpEvent, _, cx| {
                            if let Some(drag) = this.drag.take()
                                && !drag.moved
                                && distance(this.local(event.position), drag.start) <= 4.
                            {
                                let hit = this.hit(this.local(event.position));
                                if hit == drag.target {
                                    match hit {
                                        Some(Hit::Node(i)) => this.choose(i, true, cx),
                                        Some(Hit::Edge(i)) => this.expand(i, cx),
                                        Some(Hit::Collapse(i)) => {
                                            let segment = &this.tree.segments[i];
                                            let from = this.tree.rows[segment.from].id.clone();
                                            let to = this.tree.rows[segment.to].id.clone();
                                            this.collapse(&from, &to, cx);
                                        }
                                        None => {}
                                    }
                                }
                            }
                            this.pointer = this.local(event.position);
                            this.hover = this.hit(this.pointer);
                            cx.notify();
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.drag = None;
                            cx.notify();
                        }),
                    )
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if !hovered {
                            this.hover = None;
                            cx.notify();
                        }
                    }))
                    .child(
                        canvas(
                            move |bounds, window, cx| {
                                draw.update(cx, |this, cx| this.prepare(bounds, window, cx))
                            },
                            move |_, mut frame: Frame, window, cx| {
                                window.with_content_mask(Some(frame.mask), |window| {
                                    paint.read(cx).paint_lines(&frame.lines, window);
                                    for element in &mut frame.elements {
                                        element.paint(window, cx);
                                    }
                                });
                            },
                        )
                        .size_full(),
                    ),
            )
            .child(h_flex().w_full().justify_center().p_2().child(toolbar))
    }
}

fn count_text(cx: &App, key: &str, count: usize) -> String {
    let mut args = FluentArgs::new();
    args.set("count", count as i64);
    t_with_args(cx, key, &args)
}

impl HistoryCanvas {
    fn prepare(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Frame {
        let old_center = self.center();
        let resized = self.bounds.size != bounds.size;
        self.bounds = bounds;
        if self.initialized && resized {
            self.camera.pan += self.center() - old_center;
            self.hover = None;
        }
        if !self.initialized && !self.tree.nodes.is_empty() && bounds.size.height > px(0.) {
            let selected = self
                .tree
                .current
                .as_ref()
                .and_then(|id| self.tree.node_for(id))
                .or_else(|| self.tree.selected_node())
                .unwrap_or(0);
            self.camera.pan =
                self.center() - point(self.tree.nodes[selected].x, self.tree.nodes[selected].y);
            self.initialized = true;
        }
        let mask = ContentMask {
            bounds: bounds.intersect(&window.content_mask().bounds),
        };
        let (mut visible, edges) = self.visible();
        let mut elements = vec![];
        let mut lines = vec![];
        let mut label_rects: Vec<Bounds<f32>> = visible
            .iter()
            .map(|&i| {
                let radius = ((20. * self.camera.zoom).clamp(14., 32.) + 4.).max(24.) / 2.;
                Bounds::new(
                    self.position(i) - point(radius, radius),
                    size(radius * 2., radius * 2.),
                )
            })
            .collect();
        self.labels.clear();
        self.collapse_controls.clear();
        for &i in &edges {
            let edge = &self.tree.edges[i];
            let highlighted = self.tree.highlighted.contains(&edge.to);
            let color = if highlighted || self.hover == Some(Hit::Edge(i)) {
                cx.theme().primary
            } else {
                cx.theme().muted_foreground.opacity(0.4)
            };
            let points = self.edge_curve(i);
            lines.push((points.to_vec(), !edge.hidden.is_empty(), color));
            if !edge.hidden.is_empty() {
                let p = points[16];
                if p.x < -18.
                    || p.y < -10.
                    || p.x > f32::from(bounds.size.width) + 18.
                    || p.y > f32::from(bounds.size.height) + 10.
                {
                    continue;
                }
                let action_owner = cx.entity().downgrade();
                let action_id = self.tree.rows[self.tree.nodes[edge.to].row].id.clone();
                let number = edge.hidden.len().to_string();
                let control = disclosure_bounds(p, edge.hidden.len());
                label_rects.push(control);
                let label = div()
                    .id(format!(
                        "canvas-count-{}",
                        self.tree.rows[self.tree.nodes[edge.to].row].id
                    ))
                    .w(px(control.size.width))
                    .h(px(control.size.height))
                    .gap_1()
                    .rounded_sm()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().background)
                    .text_color(if highlighted || self.hover == Some(Hit::Edge(i)) {
                        cx.theme().primary
                    } else {
                        cx.theme().muted_foreground
                    })
                    .text_size(px(11.))
                    .role(Role::Button)
                    .aria_label(count_text(cx, "history-canvas-expand", edge.hidden.len()))
                    .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                        let _ = action_owner.update(cx, |this, cx| {
                            if let Some(i) = this.tree.edges.iter().position(|e| {
                                this.tree.rows[this.tree.nodes[e.to].row].id == action_id
                                    && !e.hidden.is_empty()
                            }) {
                                this.expand(i, cx);
                            }
                        });
                    })
                    .child(Icon::new(IconName::ChevronDown).size_3())
                    .child(number)
                    .into_any_element();
                self.place(
                    label,
                    control.origin,
                    control.size.map(px),
                    &mut elements,
                    mask,
                    window,
                    cx,
                );
            }
        }
        // The inverse action occupies the same incoming-link gap as the
        // collapsed count. A pale bracket identifies the expanded process range.
        let mut direct_incoming = vec![false; self.tree.nodes.len()];
        for edge in &self.tree.edges {
            direct_incoming[edge.to] = edge.hidden.is_empty();
        }
        for segment_index in 0..self.tree.segments.len() {
            let count = self.tree.collapse_count(segment_index);
            if count == 0 {
                continue;
            }
            let segment = &self.tree.segments[segment_index];
            let process_nodes: Vec<_> = segment
                .inner
                .iter()
                .filter_map(|&row| self.tree.node_for(&self.tree.rows[row].id))
                .collect();
            if let (Some(&first), Some(&last)) = (process_nodes.first(), process_nodes.last()) {
                let first = self.position(first);
                let last = self.position(last);
                let radius = ((20. * self.camera.zoom).clamp(14., 32.) + 4.).max(24.) / 2.;
                let left = first.x + radius + 96. <= f32::from(bounds.size.width);
                let side = if left { -1. } else { 1. };
                let x = first.x + side * (radius + 10.);
                let top = first.y - radius;
                let bottom = last.y + radius;
                if x > 4.
                    && x < f32::from(bounds.size.width) - 4.
                    && bottom >= 0.
                    && top <= f32::from(bounds.size.height)
                {
                    lines.push((
                        vec![
                            point(x - side * 4., top),
                            point(x, top),
                            point(x, bottom),
                            point(x - side * 4., bottom),
                        ],
                        false,
                        cx.theme().muted_foreground.opacity(0.22),
                    ));
                }
            }
            let control = process_nodes
                .into_iter()
                .filter(|&node| direct_incoming[node])
                .find_map(|node| {
                    let parent = self.tree.nodes[node].parent?;
                    let p = curve(self.position(parent), self.position(node))[16];
                    let control = disclosure_bounds(p, count);
                    (control.left() >= 4.
                        && control.right() <= f32::from(bounds.size.width) - 4.
                        && control.top() >= 4.
                        && control.bottom() <= f32::from(bounds.size.height) - 4.
                        && !label_rects.iter().any(|rect| rect.intersects(&control)))
                    .then_some(control)
                });
            let Some(control) = control else {
                continue;
            };
            label_rects.push(control);
            self.collapse_controls.push((control, segment_index));
            let from = self.tree.rows[segment.from].id.clone();
            let to = self.tree.rows[segment.to].id.clone();
            let owner = cx.weak_entity();
            let action = div()
                .id(format!("canvas-collapse-{from}-{to}"))
                .w(px(control.size.width))
                .h(px(control.size.height))
                .flex()
                .items_center()
                .justify_center()
                .gap_1()
                .rounded_sm()
                .bg(if self.hover == Some(Hit::Collapse(segment_index)) {
                    cx.theme().accent
                } else {
                    cx.theme().background
                })
                .text_color(if self.hover == Some(Hit::Collapse(segment_index)) {
                    cx.theme().foreground
                } else {
                    cx.theme().muted_foreground
                })
                .text_size(px(11.))
                .role(Role::Button)
                .aria_label(count_text(cx, "history-canvas-collapse", count))
                .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                    let _ = owner.update(cx, |this, cx| this.collapse(&from, &to, cx));
                })
                .child(Icon::new(IconName::ChevronUp).size_3())
                .child(count.to_string())
                .into_any_element();
            self.place(
                action,
                control.origin,
                control.size.map(px),
                &mut elements,
                mask,
                window,
                cx,
            );
        }
        // Selected labels win collision checks; remaining nodes retain stable order.
        visible.sort_by_key(|&i| {
            (
                self.tree.selected_node() != Some(i),
                self.hover != Some(Hit::Node(i)),
                i,
            )
        });
        for i in visible {
            let node = &self.tree.nodes[i];
            let row = &self.tree.rows[node.row];
            let p = self.position(i);
            let selected = self.tree.selected.as_deref() == Some(row.id.as_str());
            let hover = self.hover == Some(Hit::Node(i));
            let (icon, color, role) = presentation::role(row, cx);
            let icon_size = (20. * self.camera.zoom).clamp(14., 32.);
            let hit_size = (icon_size + 4.).max(24.);
            let label_width = if selected || hover {
                180.
            } else {
                (X_GAP * self.camera.zoom - 12.).clamp(64., 180.)
            };
            let show_label = (self.camera.zoom >= 1.25
                || (selected || hover) && self.camera.zoom >= 0.65)
                && p.x >= 0.
                && p.x <= f32::from(bounds.size.width)
                && p.y >= 0.;
            let caption = show_label
                .then(|| {
                    label_bounds(
                        p,
                        hit_size / 2.,
                        label_width,
                        bounds.size.map(f32::from),
                        p.x + hit_size / 2. + 96. <= f32::from(bounds.size.width),
                        &label_rects,
                    )
                })
                .flatten();
            if let Some(caption) = caption {
                label_rects.push(caption);
                self.labels.push((caption, i));
            }
            let id = row.id.clone();
            let action_id = id.clone();
            let segments: Vec<_> = self
                .tree
                .segments_at(node.row)
                .iter()
                .map(|&s| {
                    let segment = &self.tree.segments[s];
                    (
                        self.tree.rows[segment.from].id.clone(),
                        self.tree.rows[segment.to].id.clone(),
                        self.tree.collapse_count(s),
                    )
                })
                .filter(|(_, _, count)| *count > 0)
                .collect();
            let menu_owner = cx.entity().downgrade();
            let action_owner = menu_owner.clone();
            let fork_id = id.clone();
            let forkable = self.can_fork && self.forkable.contains(&id);
            let mut item = v_flex()
                .id(format!("canvas-node-{id}"))
                .w(px(hit_size))
                .items_center()
                .aria_label(format!("{role}: {}", row.title))
                .role(Role::Button)
                .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                    let _ = action_owner.update(cx, |this, cx| {
                        if let Some(i) = this.tree.node_for(&action_id) {
                            this.choose(i, true, cx);
                        }
                    });
                })
                .context_menu(move |menu, _, cx| {
                    let mut menu = menu;
                    for (from, to, count) in &segments {
                        let owner = menu_owner.clone();
                        let from = from.clone();
                        let to = to.clone();
                        menu = menu.item(
                            PopupMenuItem::new(count_text(cx, "history-canvas-collapse", *count))
                                .on_click(move |_, _, cx| {
                                    let _ =
                                        owner.update(cx, |this, cx| this.collapse(&from, &to, cx));
                                }),
                        );
                    }
                    let owner = menu_owner.clone();
                    let id = fork_id.clone();
                    menu.item(
                        PopupMenuItem::new(t(cx, "conversation-fork"))
                            .disabled(!forkable)
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    if this.can_fork && this.forkable.contains(&id) {
                                        cx.emit(CanvasEvent::Fork(id.clone()));
                                    }
                                });
                            }),
                    )
                });
            let mut glyph = div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(if self.camera.zoom < 0.65 {
                    12.
                } else {
                    icon_size + 2.
                }))
                .rounded_full()
                .border_1()
                .border_color(if row.current {
                    color
                } else {
                    Hsla::transparent_black()
                });
            if self.camera.zoom < 0.65 {
                glyph = glyph.child(div().size(px(5.)).rounded_full().bg(color));
            } else {
                glyph = glyph
                    .bg(cx.theme().background)
                    .child(Icon::new(icon).size(px(icon_size)).text_color(color));
            }
            let marker = div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(hit_size))
                .rounded(px(5.))
                .border_1()
                .border_color(if selected {
                    cx.theme().primary
                } else {
                    Hsla::transparent_black()
                })
                .bg(if selected || hover {
                    cx.theme().accent
                } else {
                    Hsla::transparent_black()
                })
                .child(glyph);
            item = item.child(marker);
            if let Some(caption) = caption {
                let label = div()
                    .h(px(32.))
                    .w(px(caption.size.width))
                    .text_size(px(11.))
                    .line_height(px(15.))
                    .text_center()
                    .when(caption.left() >= p.x, |label| label.text_left())
                    .when(caption.right() <= p.x, |label| label.text_right())
                    .line_clamp(2)
                    .text_color(cx.theme().foreground)
                    .child(row.title.clone())
                    .into_any_element();
                self.place(
                    label,
                    caption.origin,
                    caption.size.map(px),
                    &mut elements,
                    mask,
                    window,
                    cx,
                );
            }
            self.place(
                item.into_any_element(),
                p - point(hit_size / 2., hit_size / 2.),
                size(px(hit_size), px(hit_size)),
                &mut elements,
                mask,
                window,
                cx,
            );
        }
        self.prepare_tooltip(window, cx);
        if self.tree.nodes.is_empty() {
            let element = div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .text_center()
                .child(t(cx, "history-canvas-empty"))
                .into_any_element();
            self.place(
                element,
                point(12., 32.),
                size((bounds.size.width - px(24.)).max(px(0.)), px(32.)),
                &mut elements,
                mask,
                window,
                cx,
            );
        }
        Frame {
            elements,
            lines,
            mask,
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn place(
        &self,
        mut element: AnyElement,
        at: Point<f32>,
        size: Size<Pixels>,
        elements: &mut Vec<AnyElement>,
        mask: ContentMask<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.layout_as_root(size.map(AvailableSpace::Definite), window, cx);
        window.with_content_mask(Some(mask), |window| {
            element.prepaint_at(self.bounds.origin + at.map(px), window, cx);
        });
        elements.push(element);
    }
    fn paint_lines(&self, lines: &[(Vec<Point<f32>>, bool, Hsla)], window: &mut Window) {
        for (points, dashed, color) in lines {
            let mut path = PathBuilder::stroke(px(1.5));
            if !dashed {
                path.move_to(self.bounds.origin + points[0].map(px));
                for p in &points[1..] {
                    path.line_to(self.bounds.origin + p.map(px));
                }
            } else {
                let mut travelled = 0_f32;
                for pair in points.windows(2) {
                    let length = distance(pair[0], pair[1]);
                    if length == 0. {
                        continue;
                    }
                    let direction = (pair[1] - pair[0]) / length;
                    let clip = clip_segment(
                        pair[0],
                        pair[1],
                        point(-2., -2.),
                        point(
                            f32::from(self.bounds.size.width) + 2.,
                            f32::from(self.bounds.size.height) + 2.,
                        ),
                    );
                    let Some((start, end)) = clip else {
                        travelled += length;
                        continue;
                    };
                    let mut cursor = start * length;
                    let end = end * length;
                    while cursor < end {
                        let phase = (travelled + cursor) % 8.;
                        let draw = phase < 4.;
                        let remaining = if draw { 4. - phase } else { 8. - phase };
                        let step = remaining.min(end - cursor);
                        if draw {
                            path.move_to(
                                self.bounds.origin + (pair[0] + direction * cursor).map(px),
                            );
                            path.line_to(
                                self.bounds.origin
                                    + (pair[0] + direction * (cursor + step)).map(px),
                            );
                        }
                        if step < f32::EPSILON || cursor + step == cursor {
                            break;
                        }
                        cursor += step;
                    }
                    travelled += length;
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, *color);
            }
        }
    }
}
