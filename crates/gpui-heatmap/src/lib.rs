#![forbid(unsafe_code)]

//! Reusable heatmap components for GPUI applications.
//!
//! This crate owns heatmap presentation and interaction. Consumers remain
//! responsible for querying, aggregating, and assigning meaning to the values
//! they provide.

mod activity;

pub use activity::{
    ActivityHeatmap, ActivityHeatmapLabels, ActivityHeatmapSeries, ActivityHeatmapSeriesError,
};
