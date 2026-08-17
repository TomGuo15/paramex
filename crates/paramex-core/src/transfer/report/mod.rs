//! Result reporting: typed application projection, column schema, aggregate
//! stats, formatters, "Overall" rows, and byte-exact sectioned CSV export. The
//! serialization implementation remains a faithful port of
//! `gui/result_table_schema.py` + `gui/exporter.py` + `core/formatting.py` +
//! `gui/formatting.py` + the pure helpers of `gui/table_rendering.py`.

pub(super) mod csv;
mod format;
pub(super) mod output;
mod projection;
mod schema;
mod stats;
mod table;

pub(in crate::transfer) use projection::project_results_table;
pub use projection::{
    ResultsTableCell, ResultsTableColumn, ResultsTableProjection, ResultsTableRow,
    ResultsTableRowKind, ResultsTableSweep,
};

#[cfg(test)]
mod tests;
