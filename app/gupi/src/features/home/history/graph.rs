use super::*;

pub(super) const ROW_HEIGHT: f32 = 32.;
pub(super) const LANE_WIDTH: f32 = 18.;
pub(super) const VISIBLE_LANES: usize = 5;
const PADDING: f32 = 14.;

pub(super) fn column_width(lanes: usize) -> f32 {
    PADDING * 2. + lanes.clamp(2, VISIBLE_LANES).saturating_sub(1) as f32 * LANE_WIDTH
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_row(
    graph: &HistoryGraph,
    row: usize,
    start: usize,
    current: bool,
    preview: bool,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &App,
) {
    let theme = cx.theme();
    let colors = [
        theme.magenta,
        theme.cyan,
        theme.blue,
        theme.green,
        theme.yellow,
        theme.red,
    ];
    let x = |lane: usize| bounds.left() + px(PADDING + (lane as f32 - start as f32) * LANE_WIDTH);
    let center = bounds.center().y;
    window.paint_layer(bounds, |window| {
        for edge in graph.row_edges(row, start, VISIBLE_LANES) {
            let from_x = x(edge.lane);
            let parent_x = x(edge.parent_lane);
            let from_y = if edge.from == row {
                center
            } else {
                bounds.top()
            };
            let to_y = if edge.to == row {
                center
            } else {
                bounds.bottom()
            };
            let bend = edge.from == row && edge.lane != edge.parent_lane;
            let straight_start = if bend { from_y + px(8.) } else { from_y };
            let mut path = PathBuilder::stroke(px(1.5));
            if edge.indirect {
                // Fixed row coordinates keep the dash phase continuous across
                // virtual rows. A dashed edge skips filtered entries.
                let mut y = straight_start;
                while y < to_y {
                    path.move_to(point(from_x, y));
                    path.line_to(point(from_x, (y + px(3.)).min(to_y)));
                    y += px(5.5);
                }
            } else {
                path.move_to(point(from_x, straight_start));
                path.line_to(point(from_x, to_y));
            }
            if bend {
                let direction = if parent_x > from_x { 1. } else { -1. };
                path.move_to(point(from_x, straight_start));
                path.curve_to(
                    point(from_x + px(8. * direction), from_y),
                    point(from_x, from_y),
                );
                path.line_to(point(parent_x, from_y));
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, colors[edge.lane % colors.len()]);
            }
        }
        if let Some(&lane) = graph.nodes.get(row)
            && (start..start + VISIBLE_LANES).contains(&lane)
        {
            let color = colors[lane % colors.len()];
            let circle = |radius: f32| {
                Bounds::new(
                    point(x(lane) - px(radius), center - px(radius)),
                    size(px(radius * 2.), px(radius * 2.)),
                )
            };
            if current {
                window.paint_quad(fill(circle(6.), color).corner_radii(px(6.)));
                window.paint_quad(fill(circle(4.5), theme.background).corner_radii(px(4.5)));
            }
            window.paint_quad(fill(circle(3.), color).corner_radii(px(3.)));
            if preview {
                window.paint_quad(
                    outline(circle(8.), theme.foreground, BorderStyle::Solid).corner_radii(px(2.)),
                );
            }
        }
    });
}
