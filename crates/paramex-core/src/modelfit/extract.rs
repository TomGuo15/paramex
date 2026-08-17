//! UMEM transfer extraction and composition of Model Fit extraction stages.

use super::types::{Polarity, SubthresholdParams};
use crate::shared::curve_metrics::{fit_subthreshold_auto, on_off_ratio};
use crate::shared::numerics::{linear_fit_with_r2, median};

mod output;

pub(super) use output::{extract_output, output_tail_slope, output_tail_targets};

/// A transfer branch normalized to an increasing n-channel-like frame together
/// with the measured channel polarity that produced it.
pub(super) struct PreparedTransfer {
    vg: Vec<f64>,
    id: Vec<f64>,
    polarity: Polarity,
}

impl PreparedTransfer {
    pub(super) fn vg(&self) -> &[f64] {
        &self.vg
    }

    pub(super) fn id(&self) -> &[f64] {
        &self.id
    }

    pub(super) fn polarity(&self) -> Polarity {
        self.polarity
    }

    /// Preserve normalization and polarity while uniformly bounding fit work.
    pub(super) fn decimated(mut self, cap: usize) -> Self {
        (self.vg, self.id) = decimate_for_fit(&self.vg, &self.id, cap);
        self
    }
}

/// Running trapezoidal integral of `y` over `x`, mirroring
/// `scipy.integrate.cumulative_trapezoid(y, x, initial=0)`.
fn cumulative_trapezoid(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len().min(y.len());
    let mut integral = Vec::with_capacity(n);
    if n == 0 {
        return integral;
    }
    integral.push(0.0);
    let mut accumulated = 0.0;
    for index in 1..n {
        accumulated += (x[index] - x[index - 1]) * (y[index] + y[index - 1]) / 2.0;
        integral.push(accumulated);
    }
    integral
}

/// Prepare a measured transfer sweep for the (n-channel) UMEM extractors: take
/// the longest monotonic branch of a (possibly dual / hysteresis) sweep, detect
/// the channel polarity, and return the branch normalized to an **increasing,
/// n-channel-like** frame (gate axis flipped for a p-channel device), trim any
/// off-side leakage upturn before the current valley, and retain the result with
/// its detected [`Polarity`]. The extractors then run unchanged; the caller flips
/// `VT` back to the device frame via [`Polarity::map_vg`].
///
/// `None` if the sweep is too short.
pub(super) fn prepare_transfer(vg: &[f64], id_abs: &[f64]) -> Option<PreparedTransfer> {
    let n = vg.len().min(id_abs.len());
    if n < 4 {
        return None;
    }
    let (lo, hi) = longest_monotonic_run(&vg[..n]);
    let mut bvg = vg[lo..=hi].to_vec();
    let mut bid = id_abs[lo..=hi].to_vec();
    if bvg.len() < 4 {
        return None;
    }
    let polarity = detect_polarity(&bvg, &bid);
    // Flip the gate axis for a p-channel so the on-state sits at high Vg.
    if polarity == Polarity::PChannel {
        for v in &mut bvg {
            *v = -*v;
        }
    }
    // Orient increasing in the normalized gate voltage so the H-function
    // integrates from the off-state upward.
    if bvg.first() > bvg.last() {
        bvg.reverse();
        bid.reverse();
    }
    // The H integral must start at the channel-current floor. Real TFT sweeps can
    // have a gate-leakage upturn at the far-off end; integrating that unrelated
    // branch makes H decrease and can yield a negative slope / nonphysical fit.
    if let Some(valley) = bid
        .iter()
        .enumerate()
        .filter(|(_, current)| current.is_finite())
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map(|(index, _)| index)
        .filter(|&index| bvg.len() - index >= 4)
    {
        bvg.drain(..valley);
        bid.drain(..valley);
    }
    Some(PreparedTransfer {
        vg: bvg,
        id: bid,
        polarity,
    })
}

/// Point cap for the geometry/bias-dependent Level 62 LM optimizer. Kept above
/// the densest reference sweep (181 points) so the goldens stay byte-identical.
pub(super) const FIT_POINT_CAP: usize = 256;

/// Uniformly subsample a prepared monotonic transfer branch to at most `cap`
/// points. Endpoints are always kept; curves at or under `cap` are unchanged.
fn decimate_for_fit(vg: &[f64], id: &[f64], cap: usize) -> (Vec<f64>, Vec<f64>) {
    let n = vg.len().min(id.len());
    if n <= cap || cap < 2 {
        return (vg[..n].to_vec(), id[..n].to_vec());
    }
    (0..cap)
        .map(|k| {
            let index = k * (n - 1) / (cap - 1);
            (vg[index], id[index])
        })
        .unzip()
}

/// Index range `[lo, hi]` (inclusive) of the longest run over which `vg` is
/// monotonic (non-strict; flats extend the current run).
fn longest_monotonic_run(vg: &[f64]) -> (usize, usize) {
    if vg.len() < 2 {
        return (0, vg.len().saturating_sub(1));
    }
    let (mut best_lo, mut best_hi) = (0usize, 0usize);
    let mut run_lo = 0usize;
    let mut direction = 0i8;
    for index in 1..vg.len() {
        let delta = vg[index] - vg[index - 1];
        let sign = if delta > 0.0 {
            1
        } else if delta < 0.0 {
            -1
        } else {
            0
        };
        if sign == 0 {
            continue;
        }
        if direction == 0 {
            direction = sign;
        } else if sign != direction {
            if index - 1 - run_lo >= best_hi - best_lo {
                (best_lo, best_hi) = (run_lo, index - 1);
            }
            run_lo = index - 1;
            direction = sign;
        }
    }
    if (vg.len() - 1) - run_lo >= best_hi - best_lo {
        (best_lo, best_hi) = (run_lo, vg.len() - 1);
    }
    (best_lo, best_hi)
}

/// Detect channel polarity from where the current concentrates.
fn detect_polarity(vg: &[f64], id_abs: &[f64]) -> Polarity {
    let id_min = id_abs.iter().cloned().fold(f64::INFINITY, f64::min);
    let floor = if id_min.is_finite() { id_min } else { 0.0 };
    let mut sum_i = 0.0;
    let mut sum_vi = 0.0;
    for (&voltage, &current) in vg.iter().zip(id_abs) {
        let weight = (current - floor).max(0.0);
        sum_i += weight;
        sum_vi += voltage * weight;
    }
    let vg_min = vg.iter().cloned().fold(f64::INFINITY, f64::min);
    let vg_max = vg.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let midpoint = 0.5 * (vg_min + vg_max);
    let weighted = if sum_i > 0.0 {
        sum_vi / sum_i
    } else {
        midpoint
    };
    if weighted >= midpoint {
        Polarity::NChannel
    } else {
        Polarity::PChannel
    }
}

/// Result of the above-threshold H-function extraction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AboveThresholdFit {
    /// Threshold voltage `VT` (V), the x-intercept of `H` vs `Vg`.
    pub vt: f64,
    /// Mobility exponent `gamma`, from the `H`-line slope (`1/slope - 2`).
    pub gamma: f64,
    /// Gain prefactor `K` in `Id = K * (Vg - VT)^(1 + gamma)`.
    pub k: f64,
    /// R^2 of the `H` vs `Vg` linear fit.
    pub r2: f64,
}

const ABOVE_THRESHOLD_FRACTION: f64 = 1e-2;

pub(super) fn extract_above_threshold(vg: &[f64], id: &[f64]) -> Option<AboveThresholdFit> {
    extract_above_threshold_windowed(vg, id, ABOVE_THRESHOLD_FRACTION)
}

fn extract_above_threshold_windowed(
    vg: &[f64],
    id: &[f64],
    window_fraction: f64,
) -> Option<AboveThresholdFit> {
    let n = vg.len().min(id.len());
    if n < 3 {
        return None;
    }
    let vg = &vg[..n];
    let id = &id[..n];

    let id_max = id.iter().copied().fold(0.0_f64, f64::max);
    if id_max <= 0.0 {
        return None;
    }
    let floor = id_max * window_fraction;

    let h_num = cumulative_trapezoid(vg, id);
    let mut fit_vg = Vec::new();
    let mut fit_h = Vec::new();
    for index in 0..n {
        if id[index] >= floor {
            fit_vg.push(vg[index]);
            fit_h.push(h_num[index] / id[index]);
        }
    }
    if fit_vg.len() < 2 {
        return None;
    }

    let (slope, intercept, r2, _) = linear_fit_with_r2(&fit_vg, &fit_h);
    if !slope.is_finite() || slope <= 0.0 || slope >= 1.0 || !intercept.is_finite() {
        return None;
    }
    let vt = -intercept / slope;
    let gamma = 1.0 / slope - 2.0;
    if !vt.is_finite() || !gamma.is_finite() || gamma <= -1.0 {
        return None;
    }

    let mut gains = Vec::new();
    for index in 0..n {
        if id[index] >= floor {
            let overdrive = vg[index] - vt;
            if overdrive > 0.0 {
                let candidate = id[index] / overdrive.powf(1.0 + gamma);
                if candidate.is_finite() && candidate > 0.0 {
                    gains.push(candidate);
                }
            }
        }
    }
    let k = median(&mut gains)?;
    if !k.is_finite() || k <= 0.0 {
        return None;
    }

    Some(AboveThresholdFit { vt, gamma, k, r2 })
}

pub(super) fn extract_subthreshold(vg: &[f64], id: &[f64]) -> Option<SubthresholdParams> {
    let ss = fit_subthreshold_auto(vg, id, 30, 1.0, 5, 0.9, 0.3);
    let (_, ioff, _) = on_off_ratio(id);
    if !ss.swing_mv_dec.is_finite() || !ioff.is_finite() || ioff <= 0.0 {
        return None;
    }
    Some(SubthresholdParams {
        ss_v_dec: ss.swing_mv_dec / 1000.0,
        ioff,
    })
}

#[cfg(test)]
#[path = "tests/extract.rs"]
mod extraction_tests;

#[cfg(test)]
#[path = "tests/polarity.rs"]
mod polarity_tests;

#[cfg(test)]
#[path = "tests/subthreshold.rs"]
mod subthreshold_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimate_preserves_short_curves_and_caps_long_ones() {
        let vg: Vec<f64> = (0..50).map(|index| index as f64).collect();
        let id: Vec<f64> = (0..50).map(|index| 2.0 * index as f64).collect();
        let (decimated_vg, decimated_id) = decimate_for_fit(&vg, &id, 400);
        assert_eq!((decimated_vg, decimated_id), (vg, id));

        let big: Vec<f64> = (0..5067).map(|index| index as f64).collect();
        let prepared = PreparedTransfer {
            vg: big.clone(),
            id: big,
            polarity: Polarity::NChannel,
        }
        .decimated(400);
        assert_eq!(prepared.vg.len(), 400);
        assert_eq!(prepared.vg.first(), Some(&0.0));
        assert_eq!(prepared.vg.last(), Some(&5066.0));
        assert!(prepared.vg.windows(2).all(|window| window[1] > window[0]));
        assert_eq!(prepared.vg, prepared.id);
    }
}
