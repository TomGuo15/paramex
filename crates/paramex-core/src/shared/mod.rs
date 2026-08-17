//! Shared primitives used by multiple core product seams.

pub mod curve_metrics;
mod file_identity;
pub(crate) mod grid_headers;
pub(crate) mod grid_ingest;
pub mod numerics;
pub mod numpy_compat;
pub(crate) mod output_measurement;

pub use file_identity::{normalized_file_stem, same_named_source, same_source_path};
