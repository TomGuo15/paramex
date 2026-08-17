//! Column schema + raw-cell extraction from `MetricResult`
//! (`result_table_schema.py` columns + `_value_for_column`).

mod rows;

pub(super) use rows::{results_to_rows, value_for_column, Cell};

/// Per-column formatter kind (`result_table_schema.py` `FormatterKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Formatter {
    Text,
    Number,
    Current,
    PowerOfTen,
}

/// Per-column metadata (`result_table_schema.py:28-36` `ColumnSpec`).
#[derive(Debug, Clone, Copy)]
pub(super) struct ColumnSpec {
    pub(super) key: &'static str,
    pub(super) label_plain: &'static str,
    pub(super) formatter: Formatter,
}

/// The canonical column list (`result_table_schema.py:39-78` `COLUMNS`).
pub(super) const COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "filename",
        label_plain: "File",
        formatter: Formatter::Text,
    },
    ColumnSpec {
        key: "sweep",
        label_plain: "Sweep",
        formatter: Formatter::Text,
    },
    ColumnSpec {
        key: "W_um",
        label_plain: "W (\u{00B5}m)",
        formatter: Formatter::Number,
    },
    ColumnSpec {
        key: "L_um",
        label_plain: "L (\u{00B5}m)",
        formatter: Formatter::Number,
    },
    ColumnSpec {
        key: "W_over_L",
        label_plain: "W/L",
        formatter: Formatter::Number,
    },
    ColumnSpec {
        key: "geometry_source",
        label_plain: "Geometry",
        formatter: Formatter::Text,
    },
    ColumnSpec {
        key: "Vth",
        label_plain: "VTH (V)",
        formatter: Formatter::Number,
    },
    ColumnSpec {
        key: "mu_sat",
        label_plain: "mu_sat (cm^2 V^-1 s^-1)",
        formatter: Formatter::Number,
    },
    ColumnSpec {
        key: "SS_mV_dec",
        label_plain: "SS (mV dec^-1)",
        formatter: Formatter::Number,
    },
    ColumnSpec {
        key: "Ion",
        label_plain: "Ion",
        formatter: Formatter::Current,
    },
    ColumnSpec {
        key: "Ioff",
        label_plain: "Ioff",
        formatter: Formatter::Current,
    },
    ColumnSpec {
        key: "Ion_Ioff",
        label_plain: "Ion/Ioff",
        formatter: Formatter::PowerOfTen,
    },
    ColumnSpec {
        key: "DeltaVth_hysteresis",
        label_plain: "DeltaVTH,hyst (V)",
        formatter: Formatter::Number,
    },
    ColumnSpec {
        key: "status",
        label_plain: "Status",
        formatter: Formatter::Text,
    },
    ColumnSpec {
        key: "message",
        label_plain: "Message",
        formatter: Formatter::Text,
    },
];

/// All column keys in order (`COLUMN_KEYS`).
#[cfg(test)]
pub(super) fn column_keys() -> Vec<String> {
    COLUMNS.iter().map(|c| c.key.to_string()).collect()
}

/// Column keys excluding `"sweep"` (`COLUMN_KEYS_NO_SWEEP`).
pub(super) fn column_keys_no_sweep() -> Vec<String> {
    COLUMNS
        .iter()
        .filter(|c| c.key != "sweep")
        .map(|c| c.key.to_string())
        .collect()
}

/// Look up a column spec by key (`COLUMN_BY_KEY`).
pub(super) fn column_by_key(key: &str) -> Option<&'static ColumnSpec> {
    COLUMNS.iter().find(|c| c.key == key)
}

/// Index of a key in `COLUMNS`.
pub(super) fn key_index(key: &str) -> Option<usize> {
    COLUMNS.iter().position(|c| c.key == key)
}

/// Plain-text label for a key (`PLAIN_LABELS`); the key itself if unknown.
pub(super) fn plain_label(key: &str) -> String {
    column_by_key(key)
        .map(|c| c.label_plain.to_string())
        .unwrap_or_else(|| key.to_string())
}
