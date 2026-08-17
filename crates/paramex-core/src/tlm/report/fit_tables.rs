//! TLM result.csv and sweep.csv report shapes.

use crate::tlm::types::{GroupAnalysis, TlmAnalysisResult, TlmSweepResult, VoltageSweepPoint};

use super::{fcell, write_csv};

/// Eight max+median fit cells shared by result + sweep rows (`exporter.py:_fit_quantities`).
#[allow(clippy::too_many_arguments)]
fn fit_cells(
    intercept: f64,
    rc: f64,
    slope: f64,
    r2: f64,
    m_intercept: f64,
    m_rc: f64,
    m_slope: f64,
    m_r2: f64,
) -> Vec<String> {
    vec![
        fcell(intercept), // Rcontact_script_ohm (renamed from intercept_ohm)
        fcell(rc),
        fcell(slope),
        fcell(r2),
        fcell(m_intercept), // Rcontact_median_ohm
        fcell(m_rc),
        fcell(m_slope),
        fcell(m_r2),
    ]
}

const FIT_HEADERS: [&str; 8] = [
    "Rcontact_script_ohm",
    "Rc_per_contact_ohm",
    "slope_ohm_per_um",
    "r_squared",
    "Rcontact_median_ohm",
    "Rc_per_contact_median_ohm",
    "slope_median_ohm_per_um",
    "r_squared_median",
];

fn group_fit_cells(g: &GroupAnalysis) -> Vec<String> {
    fit_cells(
        g.intercept_ohm,
        g.rc_per_contact_ohm,
        g.slope_ohm_per_um,
        g.r_squared,
        g.intercept_median_ohm,
        g.rc_per_contact_median_ohm,
        g.slope_median_ohm_per_um,
        g.r_squared_median,
    )
}

fn point_fit_cells(p: &VoltageSweepPoint) -> Vec<String> {
    fit_cells(
        p.intercept_ohm,
        p.rc_per_contact_ohm,
        p.slope_ohm_per_um,
        p.r_squared,
        p.intercept_median_ohm,
        p.rc_per_contact_median_ohm,
        p.slope_median_ohm_per_um,
        p.r_squared_median,
    )
}

/// result.csv (`exporter.py:result_rows` + to_csv): one row per group.
pub fn result_csv(result: &TlmAnalysisResult) -> Vec<u8> {
    let mut headers = vec!["group", "selected_vg"];
    headers.extend(FIT_HEADERS);
    headers.extend(["valid_lengths", "warnings"]);
    let rows = result
        .groups
        .iter()
        .map(|g| {
            let mut row = vec![g.group.clone(), fcell(g.selected_vg)];
            row.extend(group_fit_cells(g));
            row.push(g.points.len().to_string());
            row.push(g.warnings.join("; "));
            row
        })
        .collect();
    write_csv(&headers, rows)
}

/// sweep.csv (`exporter.py:sweep_rows`): one row per (group, V_G).
pub fn sweep_csv(result: &TlmSweepResult) -> Vec<u8> {
    let mut headers = vec!["group", "selected_vg"];
    headers.extend(FIT_HEADERS);
    headers.extend(["valid_lengths", "warnings"]);
    let rows = result
        .points
        .iter()
        .map(|p| {
            let mut row = vec![p.group.clone(), fcell(p.selected_vg)];
            row.extend(point_fit_cells(p));
            row.push(p.valid_lengths.to_string());
            row.push(p.warnings.join("; "));
            row
        })
        .collect();
    write_csv(&headers, rows)
}
