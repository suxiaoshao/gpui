use gpui_kit::{Bounds, Point, Size, point, size};

#[derive(Clone, Copy, Debug)]
pub(super) struct Camera {
    pub zoom: f32,
    pub pan: Point<f32>,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 1.,
            pan: point(0., 0.),
        }
    }
}
impl Camera {
    pub fn screen(self, p: Point<f32>) -> Point<f32> {
        p * self.zoom + self.pan
    }
    pub fn world(self, p: Point<f32>) -> Point<f32> {
        (p - self.pan) / self.zoom
    }
    pub fn zoom_at(&mut self, anchor: Point<f32>, zoom: f32) {
        let world = self.world(anchor);
        self.zoom = zoom.clamp(0.001, 4.);
        self.pan = anchor - world * self.zoom;
    }
}

/// Same cubic geometry is used for painting, labels and pointer distance.
pub(super) fn curve(a: Point<f32>, b: Point<f32>) -> [Point<f32>; 33] {
    let c = point(a.x, (a.y + b.y) / 2.);
    let d = point(b.x, c.y);
    std::array::from_fn(|i| {
        let t = i as f32 / 32.;
        let u = 1. - t;
        a * (u * u * u) + c * (3. * u * u * t) + d * (3. * u * t * t) + b * (t * t * t)
    })
}
pub(super) fn distance(a: Point<f32>, b: Point<f32>) -> f32 {
    let d = a - b;
    (d.x * d.x + d.y * d.y).sqrt()
}
pub(super) fn path_distance(p: Point<f32>, points: &[Point<f32>]) -> f32 {
    points
        .windows(2)
        .map(|w| {
            let v = w[1] - w[0];
            let u = p - w[0];
            let length = v.x * v.x + v.y * v.y;
            let t = if length == 0. {
                0.
            } else {
                ((u.x * v.x + u.y * v.y) / length).clamp(0., 1.)
            };
            distance(p, w[0] + v * t)
        })
        .fold(f32::INFINITY, f32::min)
}

/// Parametric interval inside the viewport; clip before generating screen-sized dashes.
pub(super) fn clip_segment(
    a: Point<f32>,
    b: Point<f32>,
    low: Point<f32>,
    high: Point<f32>,
) -> Option<(f32, f32)> {
    let delta = b - a;
    let (mut start, mut end) = (0_f32, 1_f32);
    for (origin, step, min, max) in [(a.x, delta.x, low.x, high.x), (a.y, delta.y, low.y, high.y)] {
        if step.abs() < f32::EPSILON {
            if origin < min || origin > max {
                return None;
            }
        } else {
            let p = (min - origin) / step;
            let q = (max - origin) / step;
            start = start.max(p.min(q));
            end = end.min(p.max(q));
            if start > end {
                return None;
            }
        }
    }
    Some((start, end))
}

pub(super) fn disclosure_bounds(p: Point<f32>, count: usize) -> Bounds<f32> {
    let width = (count.to_string().len() as f32 * 7. + 26.).max(36.);
    Bounds::new(p - point(width / 2., 12.), size(width, 24.))
}

pub(super) fn label_bounds(
    p: Point<f32>,
    radius: f32,
    width: f32,
    viewport: Size<f32>,
    prefer_right: bool,
    occupied: &[Bounds<f32>],
) -> Option<Bounds<f32>> {
    let below = point(
        (p.x - width / 2.).clamp(8., (viewport.width - width - 8.).max(8.)),
        p.y + radius + 4.,
    );
    let available = if prefer_right {
        viewport.width - p.x - radius - 16.
    } else {
        p.x - radius - 16.
    };
    let side_width = width.min(available.max(0.));
    let side = Bounds::new(
        point(
            if prefer_right {
                p.x + radius + 8.
            } else {
                p.x - radius - 8. - side_width
            },
            p.y - 16.,
        ),
        size(side_width, 32.),
    );
    let above = point(below.x, p.y - radius - 36.);
    // A chain keeps its preferred side even when one caption needs more room.
    // Collisions fall back vertically, never to the opposite side of the node.
    let mut candidates = (side_width >= 64.).then_some(side).into_iter().chain([
        Bounds::new(below, size(width, 32.)),
        Bounds::new(above, size(width, 32.)),
    ]);
    candidates.find(|b| {
        b.left() >= 8.
            && b.top() >= 4.
            && b.right() <= viewport.width - 8.
            && b.bottom() <= viewport.height - 4.
            && !occupied.iter().any(|r| r.intersects(b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_zoom_and_relayout_anchor_stay_in_screen_space() {
        let mut camera = Camera {
            zoom: 0.7,
            pan: point(140., -400.),
        };
        let pointer = point(90., 250.);
        let world = camera.world(pointer);
        camera.zoom_at(pointer, 2.1);
        assert!(distance(camera.screen(world), pointer) < 0.001);
        let new_world = world + point(500., 600.);
        camera.pan = pointer - new_world * camera.zoom;
        assert!(distance(camera.screen(new_world), pointer) < 0.001);
    }
    #[test]
    fn curved_edge_hit_width_does_not_shrink_with_zoom() {
        for zoom in [0.1, 1., 3.] {
            let p = curve(point(0., 0.), point(200. * zoom, 100. * zoom));
            assert!(path_distance(p[16] + point(0., 8.), &p) <= 8.01);
            assert!(path_distance(point(-100., -100.), &p) > 12.);
        }
    }
    #[test]
    fn long_offscreen_edges_are_clipped_even_when_both_ends_are_outside() {
        let range = clip_segment(
            point(-100_000., 20.),
            point(100_000., 20.),
            point(0., 0.),
            point(300., 600.),
        )
        .unwrap();
        assert!(((range.1 - range.0) * 200_000. - 300.).abs() < 0.02);
        assert!(
            clip_segment(
                point(-20., -20.),
                point(-10., -10.),
                point(0., 0.),
                point(300., 600.)
            )
            .is_none()
        );
    }
    #[test]
    fn fork_caption_avoids_the_fold_count_and_edge_labels_stay_inside_the_pane() {
        let count = Bounds::new(point(136., 130.), size(28., 20.));
        let caption = label_bounds(
            point(150., 100.),
            16.,
            100.,
            size(480., 600.),
            true,
            &[count],
        )
        .unwrap();
        assert!(!caption.intersects(&count));
        let edge_caption =
            label_bounds(point(28., 350.), 12., 180., size(300., 600.), false, &[]).unwrap();
        assert!(edge_caption.left() >= 8. && edge_caption.right() <= 292.);
    }
    #[test]
    fn chain_captions_keep_their_side_and_fall_back_vertically_on_collision() {
        let short = label_bounds(point(200., 100.), 12., 80., size(520., 600.), true, &[]).unwrap();
        let long = label_bounds(point(200., 200.), 12., 180., size(520., 600.), true, &[]).unwrap();
        assert_eq!(short.left(), long.left());
        assert!(short.left() > 200.);
        let collision = label_bounds(
            point(200., 200.),
            12.,
            180.,
            size(520., 600.),
            true,
            &[long],
        )
        .unwrap();
        assert!(collision.top() > 200. || collision.bottom() < 200.);
        let narrow =
            label_bounds(point(120., 200.), 12., 180., size(240., 600.), true, &[]).unwrap();
        assert!(narrow.left() > 120. && narrow.right() <= 232.);
    }
}
