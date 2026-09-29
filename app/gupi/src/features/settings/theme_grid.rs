use super::*;
use gpui_kit::component::ThemeMode;

// Settings groups need an intrinsic height. Measure it during layout, then
// render the equal-width grid at the assigned width in the same frame.
pub(super) struct ThemeGrid {
    pub group: (&'static str, ThemeMode, Vec<app_theme::ThemeChoice>),
    pub draft: AppConfig,
    pub controller: Entity<ConfigController>,
    pub busy: bool,
}

impl IntoElement for ThemeGrid {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for ThemeGrid {
    type RequestLayoutState = ();
    type PrepaintState = AnyElement;

    fn id(&self) -> Option<ElementId> {
        Some(self.group.0.into())
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        // All tiles have a fixed preview and a single-line caption. Measure one
        // real tile to retain font, border and padding metrics without hardcoding height.
        let mut sample = preferences::theme_grid_content(
            (
                self.group.0,
                self.group.1,
                self.group.2.iter().take(1).cloned().collect(),
            ),
            &self.draft,
            &self.controller,
            1,
            self.busy,
            cx,
        );
        let tile_height = sample
            .layout_as_root(
                size(
                    AvailableSpace::Definite(px(178.)),
                    AvailableSpace::MaxContent,
                ),
                window,
                cx,
            )
            .height;
        let count = self.group.2.len();
        let gap = window.rem_size() * 0.75; // gap_3, shared with the rendered grid.
        let style = Style {
            size: size(relative(1.).into(), Length::Auto),
            flex_shrink: 0.,
            ..Default::default()
        };
        let layout = window.request_measured_layout(style, move |known, available, _, _| {
            let width = known.width.unwrap_or(match available.width {
                AvailableSpace::Definite(width) => width,
                _ => px(178.),
            });
            let columns = preferences::theme_columns(width.as_f32()) as usize;
            let rows = count.div_ceil(columns);
            size(
                width,
                tile_height * rows as f32 + gap * rows.saturating_sub(1) as f32,
            )
        });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let mut grid = preferences::theme_grid_content(
            (
                self.group.0,
                self.group.1,
                std::mem::take(&mut self.group.2),
            ),
            &self.draft,
            &self.controller,
            preferences::theme_columns(bounds.size.width.as_f32()),
            self.busy,
            cx,
        );
        grid.layout_as_root(bounds.size.map(AvailableSpace::Definite), window, cx);
        grid.prepaint_at(bounds.origin, window, cx);
        grid
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        grid: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        grid.paint(window, cx);
    }
}
