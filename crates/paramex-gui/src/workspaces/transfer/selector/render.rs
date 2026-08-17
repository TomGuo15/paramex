//! Per-graph selector render pipeline.
//!
//! This module owns the repeated VT/SS column sequence: series construction,
//! plot rendering, on-chart drag capture, and numeric controls.

use eframe::egui;

use super::controls::graph_controls;
use super::drag::{grab_band, Commit};
use super::window::{draw_windows, grab_inputs, GraphSide};
use super::{bands, graph};
use crate::state::EditBuffers;
use crate::theme::{SUISEI_DARK, SUISEI_MAIN};
use crate::workspaces::transfer::state::{
    AxisRanges, CurveView, DragState, GraphMode, PlotKind, SelectorUi, SweepBranch,
};

// Direction toggle, range strip, numeric row, and their gaps. No plot footer
// follows these controls, so this is the only space withheld from the plot.
const SELECTOR_CONTROLS_RESERVE: f32 = 105.0;

/// Render one graph: bands (fwd solid, bwd dashed) + scatter + the active-direction
/// dashed fit-line (live drag -> strict `preview_gate`; committed -> weak `committed_line_gate`).
#[allow(clippy::too_many_arguments)] // disjoint per-graph render inputs
fn render_one(
    ui: &mut egui::Ui,
    view: &CurveView,
    id_source: &str,
    kind: PlotKind,
    mode: GraphMode,
    series: &[graph::SeriesDraw<'_>],
    axes: AxisRanges,
    y_bounds: [f64; 2],
    fwd_w: Option<(f64, f64)>,
    bwd_w: Option<(f64, f64)>,
    live: Option<(f64, f64)>,
    has_bwd: bool,
    show_values: bool,
) -> egui_plot::PlotResponse<()> {
    let vg_axis = axes.vg();
    let (fwd_draw, bwd_draw, active_fit) = if show_values {
        draw_windows(mode, fwd_w, bwd_w, live, vg_axis)
    } else {
        (None, None, None)
    };
    let is_live = live.is_some();
    let fit = active_fit.and_then(|(w, branch)| {
        let r = view.fitter(branch, kind).fit(Some(w));
        let ok = if is_live {
            graph::preview_gate(&r)
        } else {
            graph::committed_line_gate(&r)
        };
        if ok {
            let color = match branch {
                SweepBranch::Forward => SUISEI_MAIN,
                SweepBranch::Backward => SUISEI_DARK,
            };
            crate::plot_kit::fit_line_endpoints(r.slope, r.intercept, vg_axis.0, vg_axis.1)
                .map(|points| (points, color))
        } else {
            None
        }
    });
    let mut draws = Vec::new();
    if let Some(w) = fwd_draw {
        draws.push(graph::BandDraw {
            window: w,
            fill: bands::forward_fill(),
            stroke: SUISEI_MAIN,
        });
    }
    if has_bwd {
        if let Some(w) = bwd_draw {
            draws.push(graph::BandDraw {
                window: w,
                fill: bands::backward_fill(),
                stroke: SUISEI_DARK,
            });
        }
    }
    // Return the PlotResponse so callers can hit-test the on-chart bands.
    // Axis titles name the physical quantity and unit; log10 is only the SS plot's
    // internal coordinate transform.
    let (title, y_label) = match kind {
        PlotKind::Vt => (
            "V<sub>TH</sub> fit range",
            "\u{221A}|I<sub>D</sub>| (A<sup>1/2</sup>)",
        ),
        PlotKind::Ss => ("SS fit range", "|I<sub>D</sub>| (A)"),
    };
    // Fill the card vertically: reserve exactly the per-graph controls rendered
    // below this plot. The legend row is gone and axis titles live
    // inside the plot, so there is no footer reserve after the terminal fields.
    let plot_h = selector_plot_height(ui.available_height());
    graph::render_graph(
        ui,
        id_source,
        title,
        "Gate voltage V<sub>G</sub> (V)",
        y_label,
        vg_axis,
        y_bounds,
        matches!(kind, PlotKind::Ss),
        plot_h,
        show_values,
        series,
        fit,
        &draws,
    )
}

fn selector_plot_height(available: f32) -> f32 {
    (available - SELECTOR_CONTROLS_RESERVE).max(0.0)
}

/// Drop this file's selector edit buffers so the numeric fields re-sync to the
/// committed windows (the Python `_suppress` equivalent on file-switch).
/// Uses the actual `GraphSide` enum + `{:?}` format to match `graph_controls`'s key
/// format exactly, so the keys can't silently drift if a variant is renamed.
pub(super) fn forget_selector_buffers(edits: &mut EditBuffers, id: &str) {
    for side in [GraphSide::Vt, GraphSide::Ss] {
        edits.forget(&format!("num:{id}:{side:?}:lo"));
        edits.forget(&format!("num:{id}:{side:?}:hi"));
    }
}

/// One selector column (a single graph): build the series draws from the cached
/// view -> `render_one` -> `grab_inputs` + `grab_band` -> `graph_controls`. Both VT
/// and SS columns ran this exact sequence; factoring it out removes the
/// dual-maintenance hazard (every per-fix tweak previously had to be made twice).
/// `mode` is not a parameter: it is read from `sel` inside, via the same
/// `side`-keyed match `graph_controls` uses, so there is no E0503 (reading a Copy
/// field off the `&mut sel` we already hold). `live` and `drag_at_start` stay
/// precomputed parameters: recomputing `live` here would let the SS column observe
/// the VT column's `grab_band` mutation of selector drag state, which would be a behavior
/// change.
#[allow(clippy::too_many_arguments)] // reason: disjoint per-graph render inputs (matches render_one's neighbours)
pub(super) fn render_selector_column(
    ui: &mut egui::Ui,
    view: &CurveView,
    sel: &mut SelectorUi,
    edits: &mut EditBuffers,
    commits: &mut Vec<Commit>,
    id: &str,
    side: GraphSide,
    kind: PlotKind,
    id_source: &str,
    axes: AxisRanges,
    y_bounds: [f64; 2],
    w: Option<(f64, f64)>,
    wb: Option<(f64, f64)>,
    live: Option<(f64, f64)>,
    has_bwd: bool,
    show_values: bool,
    drag_at_start: Option<DragState>,
) {
    // Read mode by value (GraphMode: Copy) from the `&mut sel` we hold - same match
    // graph_controls uses. Nothing mutates graph mode before graph_controls runs
    // (grab_band only touches selector drag state), so this is behavior-identical
    // to the old code that passed the modes positionally.
    let mode = sel.mode(side.plot_kind());
    let mut series = Vec::new();
    if show_values {
        series.push(graph::SeriesDraw {
            name: "Forward sweep",
            points: view.scatter(SweepBranch::Forward, kind),
            color: SUISEI_MAIN,
        });
    }
    // The bwd scatter is always cached (result-independent); drawing stays gated
    // on the committed result's has_backward_sweep, exactly as before.
    if show_values && has_bwd {
        series.push(graph::SeriesDraw {
            name: "Backward sweep",
            points: view.scatter(SweepBranch::Backward, kind),
            color: SUISEI_DARK,
        });
    }
    let resp = render_one(
        ui,
        view,
        id_source,
        kind,
        mode,
        &series,
        axes,
        y_bounds,
        w,
        wb,
        live,
        has_bwd,
        show_values,
    );
    if show_values {
        let (sorted_xs, committed_window) = grab_inputs(view, mode, kind, w, wb);
        grab_band(
            resp,
            side,
            mode,
            committed_window,
            sorted_xs,
            id,
            sel,
            commits,
        );
    }
    graph_controls(
        ui,
        id,
        side,
        sel,
        drag_at_start,
        w,
        wb,
        has_bwd,
        axes.vg(),
        show_values,
        edits,
        commits,
    );
}
