use super::{
    AnalogFitQuality, FitModel, FittedModelView, ModelCardArtifact, OutputSeries,
    GM_ID_CEILING_V_INV,
};
use crate::modelfit::ac;
use crate::modelfit::export::{model_card_filename, verilog_a_card};
use crate::modelfit::extract::{output_tail_slope, prepare_transfer};
use crate::modelfit::forward::unified_transfer;
use crate::modelfit::level62::{level62_output, level62_transfer, level62_verilog_a_card};
use crate::modelfit::types::SubthresholdParams;
use crate::shared::numpy_compat::{gradient, interp};

impl<'a> FittedModelView<'a> {
    pub fn r2(&self) -> Option<f64> {
        match self.kind {
            FitModel::Aostft => Some(self.device.aostft.h_fit.r2),
            FitModel::Level62 => self.device.level62.fit.as_ref().map(|f| f.r2),
        }
        .filter(|v| v.is_finite())
    }
    pub fn is_manual(&self) -> bool {
        match self.kind {
            FitModel::Aostft => self.device.aostft.manual,
            FitModel::Level62 => self.device.level62.manual,
        }
    }
    pub fn transfer_overlay(&self) -> Vec<[f64; 2]> {
        self.device.modelled_points(self.kind)
    }
    /// AOSTFT has a complete card with documented defaults when its H fit can be
    /// projected into the strict finite-drain representation; Level 62 needs an
    /// extracted Level 62 fit.
    pub fn is_export_ready(&self) -> bool {
        match self.kind {
            FitModel::Aostft => self.device.aostft_card_projection().is_some(),
            FitModel::Level62 => self.device.level62.fit.is_some(),
        }
    }
    pub fn export_artifact(&self) -> Option<ModelCardArtifact> {
        let text = match self.kind {
            FitModel::Aostft => {
                let projection = self.device.aostft_card_projection()?;
                verilog_a_card(
                    &self.device.name,
                    &projection.fit,
                    Some(&projection.output),
                    self.device.aostft.subthreshold.as_ref(),
                    self.device.geometry,
                    self.device.bias,
                    self.device.polarity,
                )
            }
            FitModel::Level62 => {
                let fit = self.device.level62.fit.as_ref()?;
                level62_verilog_a_card(
                    &self.device.name,
                    &fit.params,
                    self.device.geometry,
                    fit.params.tnom_k,
                    fit.polarity,
                )
            }
        };
        Some(ModelCardArtifact {
            text,
            suggested_file_name: model_card_filename(&self.device.name),
        })
    }
    pub fn output_family(&self) -> Vec<OutputSeries> {
        if self.device.output_curves.is_empty() {
            return self.predicted_output_family();
        }
        let s = self.device.polarity.sign();
        self.device
            .output_curves
            .iter()
            .map(|c| {
                let measured = c.vds.iter().zip(&c.id).map(|(&v, &i)| [s * v, i]).collect();
                let modelled = match self.kind {
                    FitModel::Aostft => match self.device.aostft.output {
                        Some(_) if self.device.bias.v_ds > 0.0 => {
                            let projection = self
                                .device
                                .aostft_card_projection()
                                .expect("AOSTFT output mutations preserve a valid card projection");
                            let vov = self.device.polarity.map_vg(c.vg)
                                - self.device.polarity.map_vg(projection.fit.vt);
                            let vd: Vec<_> = c.vds.iter().map(|&v| s * v).collect();
                            curve(&vd, |vd| {
                                self.device.card_output_with_fit(
                                    projection.fit,
                                    projection.output,
                                    vov,
                                    vd,
                                )
                            })
                        }
                        _ => Vec::new(),
                    },
                    FitModel::Level62 => match &self.device.level62.fit {
                        Some(fit) => {
                            let vg = self.device.polarity.map_vg(c.vg);
                            let vd: Vec<_> = c.vds.iter().map(|&v| s * v).collect();
                            curve(&vd, |vd| {
                                level62_output(
                                    &fit.params,
                                    self.device.geometry,
                                    fit.params.tnom_k,
                                    vg,
                                    vd,
                                )
                            })
                        }
                        None => Vec::new(),
                    },
                };
                OutputSeries {
                    vg: c.vg,
                    measured,
                    modelled,
                }
            })
            .collect()
    }
    fn predicted_output_family(&self) -> Vec<OutputSeries> {
        let aostft_card = match self.kind {
            FitModel::Aostft => match self.device.aostft_card_projection() {
                Some(card) => Some(card),
                None => return Vec::new(),
            },
            FitModel::Level62 => None,
        };
        let turn_on = match self.kind {
            FitModel::Aostft => self
                .device
                .polarity
                .map_vg(aostft_card.as_ref().unwrap().fit.vt),
            FitModel::Level62 => match &self.device.level62.fit {
                Some(f) => f.params.vto,
                None => return Vec::new(),
            },
        };
        let max = self
            .device
            .vgs
            .iter()
            .map(|&v| self.device.polarity.map_vg(v))
            .fold(f64::NEG_INFINITY, f64::max)
            - turn_on;
        if !(max.is_finite() && max > 0.0) {
            return Vec::new();
        }
        let vd: Vec<_> = (0..=60).map(|i| i as f64 / 60.0 * max).collect();
        let sign = self.device.polarity.sign();
        [0.25, 0.5, 0.75, 1.0]
            .iter()
            .map(|&f| {
                let vg = turn_on + f * max;
                let modelled = match self.kind {
                    FitModel::Aostft => {
                        if self.device.bias.v_ds <= 0.0 {
                            Vec::new()
                        } else {
                            let projection = aostft_card.unwrap();
                            curve(&vd, |x| {
                                self.device.card_output_with_fit(
                                    projection.fit,
                                    projection.output,
                                    vg - turn_on,
                                    x,
                                )
                            })
                        }
                    }
                    FitModel::Level62 => {
                        let fit = self.device.level62.fit.as_ref().unwrap();
                        curve(&vd, |x| {
                            level62_output(
                                &fit.params,
                                self.device.geometry,
                                fit.params.tnom_k,
                                vg,
                                x,
                            )
                        })
                    }
                };
                OutputSeries {
                    vg: sign * vg,
                    measured: Vec::new(),
                    modelled,
                }
            })
            .collect()
    }
    fn transfer_on_grid(&self, vg: &[f64]) -> Option<Vec<f64>> {
        let vgn: Vec<_> = vg.iter().map(|&v| self.device.polarity.map_vg(v)).collect();
        match self.kind {
            FitModel::Aostft => {
                let sub = self
                    .device
                    .aostft
                    .subthreshold
                    .unwrap_or_else(SubthresholdParams::card_defaults);
                let h_fit = self.device.aostft.h_fit;
                let card = self.device.aostft.output.map(|_| {
                    self.device
                        .aostft_card_projection()
                        .expect("AOSTFT output mutations preserve a valid card projection")
                });
                let displayed_fit = card.map_or(h_fit, |projection| projection.fit);
                let vt = self.device.polarity.map_vg(displayed_fit.vt);
                Some(match card {
                    Some(projection) if self.device.bias.v_ds > 0.0 => vgn
                        .iter()
                        .map(|&v| {
                            self.device.card_output_with_fit(
                                projection.fit,
                                projection.output,
                                v - vt,
                                &[self.device.bias.v_ds],
                            )[0]
                        })
                        .collect(),
                    _ => unified_transfer(vt, h_fit.gamma, h_fit.k, &sub, &vgn),
                })
            }
            FitModel::Level62 => self.device.level62.fit.as_ref().map(|f| {
                level62_transfer(
                    &f.params,
                    self.device.geometry,
                    f.params.tnom_k,
                    &vgn,
                    self.device.bias.v_ds,
                )
            }),
        }
    }
    fn vg_grid(&self) -> Vec<f64> {
        let lo = self
            .device
            .vgs
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let hi = self
            .device
            .vgs
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        if !(lo.is_finite() && hi.is_finite() && hi > lo) {
            return Vec::new();
        }
        (0..200)
            .map(|i| lo + (hi - lo) * i as f64 / 199.0)
            .collect()
    }
    pub fn gm_series(&self) -> Vec<[f64; 2]> {
        let vg = self.vg_grid();
        let Some(id) = self.transfer_on_grid(&vg) else {
            return Vec::new();
        };
        let vgn: Vec<_> = vg.iter().map(|&v| self.device.polarity.map_vg(v)).collect();
        gradient(&id, &vgn)
            .into_iter()
            .zip(vg)
            .map(|(g, v)| [v, g])
            .collect()
    }
    pub fn gds_series(&self) -> Vec<OutputSeries> {
        self.output_family()
            .into_iter()
            .filter_map(|s| {
                let deriv = |p: &[[f64; 2]]| {
                    let (x, y): (Vec<_>, Vec<_>) = p
                        .iter()
                        .filter(|p| p[0].is_finite() && p[1].is_finite())
                        .map(|p| (p[0], p[1]))
                        .unzip();
                    if x.len() < 2 {
                        Vec::new()
                    } else {
                        gradient(&y, &x)
                            .into_iter()
                            .zip(x)
                            .filter_map(|(g, x)| (g.is_finite()).then_some([x, g]))
                            .collect()
                    }
                };
                let measured = deriv(&s.measured);
                let modelled = deriv(&s.modelled);
                (measured.len() >= 2 || modelled.len() >= 2).then_some(OutputSeries {
                    vg: s.vg,
                    measured,
                    modelled,
                })
            })
            .collect()
    }
    pub fn gm_id_sizing_series(&self) -> Vec<[f64; 2]> {
        let vg = self.vg_grid();
        let Some(id) = self.transfer_on_grid(&vg) else {
            return Vec::new();
        };
        let w = self.device.geometry.w_um;
        if !(w.is_finite() && w > 0.0) {
            return Vec::new();
        }
        let vgn: Vec<_> = vg.iter().map(|&v| self.device.polarity.map_vg(v)).collect();
        let gmid = ac::gm_over_id(&gradient(&id, &vgn), &id);
        let peak = gmid
            .iter()
            .enumerate()
            .filter(|(_, x)| x.is_finite())
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(i, _)| i);
        let strong = id
            .iter()
            .enumerate()
            .filter(|(_, x)| x.is_finite())
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map_or(0, |(i, _)| i);
        let order: Vec<_> = if strong <= peak {
            (strong..=peak).collect()
        } else {
            (peak..=strong).rev().collect()
        };
        let mut out = Vec::new();
        let mut last = 0.0;
        for i in order {
            let e = gmid[i];
            let d = id[i].abs() / w;
            if !(e.is_finite() && e > 0.0 && e <= GM_ID_CEILING_V_INV && d.is_finite() && d > 0.0) {
                continue;
            }
            if e <= last {
                break;
            }
            last = e;
            out.push([e, d]);
        }
        out
    }
    pub fn intrinsic_gain_series(&self) -> Vec<[f64; 2]> {
        let turn_on = match self.kind {
            FitModel::Aostft => self.device.polarity.map_vg(self.device.aostft.h_fit.vt),
            FitModel::Level62 => match &self.device.level62.fit {
                Some(fit) => fit.params.vto,
                None => return Vec::new(),
            },
        };
        let max = self
            .device
            .vgs
            .iter()
            .map(|&v| self.device.polarity.map_vg(v))
            .fold(f64::NEG_INFINITY, f64::max)
            - turn_on;
        if !(max.is_finite() && max > 0.0) {
            return Vec::new();
        }
        let samples: Vec<_> = (0..40)
            .rev()
            .map(|i| {
                self.saturation_point(
                    self.device.polarity.sign() * (turn_on + (0.25 + 0.75 * i as f64 / 39.0) * max),
                )
            })
            .collect();
        longest_monotone_gain_branch(&samples)
    }
    fn saturation_point(&self, gate_vg: f64) -> Option<[f64; 2]> {
        let vg = self.device.polarity.map_vg(gate_vg);
        let evaluate = |vdsat: f64, output: &dyn Fn(f64, f64) -> f64| {
            if !(vdsat.is_finite() && vdsat > 0.0) {
                return None;
            }
            let vd = 2.0 * vdsat;
            let step = (vdsat * 1.0e-3).max(1.0e-6);
            let id = output(vg, vd).abs();
            let gm = (output(vg + step, vd) - output(vg - step, vd)) / (2.0 * step);
            let gds = (output(vg, vd + step) - output(vg, vd - step)) / (2.0 * step);
            let gmid = gm / id;
            let gain = gm / gds;
            (id > 0.0 && gds > 0.0 && gain.is_finite() && gmid.is_finite() && gmid > 0.0)
                .then_some([gmid, gain])
        };
        match self.kind {
            FitModel::Level62 => {
                let fit = self.device.level62.fit.as_ref()?;
                evaluate(fit.params.asat * (vg - fit.params.vto), &|vgs, vds| {
                    level62_output(
                        &fit.params,
                        self.device.geometry,
                        fit.params.tnom_k,
                        vgs,
                        &[vds],
                    )[0]
                })
            }
            FitModel::Aostft => {
                if self.device.bias.v_ds <= 0.0 {
                    return None;
                }
                let projection = self.device.aostft_card_projection()?;
                let vt = self.device.polarity.map_vg(projection.fit.vt);
                evaluate(projection.output.alpha_sat * (vg - vt), &|vgs, vds| {
                    self.device.card_output_with_fit(
                        projection.fit,
                        projection.output,
                        vgs - vt,
                        &[vds],
                    )[0]
                })
            }
        }
    }
    pub fn analog_fit_quality(&self) -> AnalogFitQuality {
        let cache = match self.kind {
            FitModel::Aostft => &self.device.aostft.analog_quality_cache,
            FitModel::Level62 => &self.device.level62.analog_quality_cache,
        };
        if let Some(q) = cache.get() {
            return q;
        }
        let q = AnalogFitQuality {
            gm_p90: self.analog_gm_p90(),
            gds_p90: self.analog_gds_p90(),
        };
        cache.set(Some(q));
        q
    }
    fn analog_gm_p90(&self) -> Option<f64> {
        let prepared = prepare_transfer(&self.device.vgs, &self.device.id)?;
        if prepared.vg().len() < 5 {
            return None;
        }
        let modelled = self.transfer_overlay();
        if modelled.len() != self.device.vgs.len()
            || modelled
                .iter()
                .any(|point| !point[0].is_finite() || !point[1].is_finite())
        {
            return None;
        }
        let mut model: Vec<_> = modelled
            .into_iter()
            .map(|point| [self.device.polarity.map_vg(point[0]), point[1]])
            .collect();
        model.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let mv: Vec<_> = model.iter().map(|p| p[0]).collect();
        let mi: Vec<_> = model.iter().map(|p| p[1]).collect();
        if mv.is_empty() {
            return None;
        }
        let pred = interp(prepared.vg(), &mv, &mi);
        if pred.iter().any(|value| !value.is_finite()) {
            return None;
        }
        let floor = prepared.id().iter().copied().fold(f64::INFINITY, f64::min);
        let peak = prepared.id().iter().copied().fold(0.0, f64::max);
        let mut pairs = Vec::new();
        for i in 2..prepared.vg().len() - 2 {
            let dv = prepared.vg()[i + 2] - prepared.vg()[i - 2];
            if dv > 0.0 && prepared.id()[i] >= floor + 0.01 * (peak - floor) {
                let m = (prepared.id()[i + 2] - prepared.id()[i - 2]) / dv;
                let p = (pred[i + 2] - pred[i - 2]) / dv;
                if m.is_finite() && m > 0.0 {
                    if !p.is_finite() {
                        return None;
                    }
                    pairs.push((m, p));
                }
            }
        }
        let mut ms: Vec<_> = pairs.iter().map(|p| p.0).collect();
        ms.sort_by(f64::total_cmp);
        let p95 = percentile(&ms, 0.95)?;
        let mut e: Vec<_> = pairs
            .into_iter()
            .filter(|p| p.0 >= 0.05 * p95)
            .map(|p| (p.1 - p.0).abs() / p.0.max(0.1 * p95))
            .collect();
        e.sort_by(f64::total_cmp);
        (e.len() >= 5).then(|| percentile(&e, 0.9)).flatten()
    }
    fn analog_gds_p90(&self) -> Option<f64> {
        if !self.device.has_output_curves() {
            return None;
        }
        let family = self.output_family();
        let peak = family
            .iter()
            .flat_map(|c| c.measured.iter().map(|p| p[1]))
            .fold(0.0, f64::max);
        let mut errors = Vec::new();
        for c in family {
            let cp = c.measured.iter().map(|p| p[1]).fold(0.0, f64::max);
            if cp < 0.2 * peak {
                continue;
            }
            let measured_xmax = c.measured.iter().map(|p| p[0]).fold(0.0, f64::max);
            let model_is_valid = c.modelled.len() == c.measured.len()
                && c.modelled.iter().flatten().all(|value| value.is_finite());
            let model_xmax = if model_is_valid {
                c.modelled.iter().map(|p| p[0]).fold(0.0, f64::max)
            } else {
                0.0
            };
            let (measured_vd, measured_id): (Vec<_>, Vec<_>) = c
                .measured
                .iter()
                .filter(|p| p[0] >= 0.6 * measured_xmax)
                .map(|p| (p[0], p[1]))
                .unzip();
            let (model_vd, model_id): (Vec<_>, Vec<_>) = c
                .modelled
                .iter()
                .filter(|p| p[0] >= 0.6 * model_xmax)
                .map(|p| (p[0], p[1]))
                .unzip();
            let Some((s, r2)) = output_tail_slope(&measured_vd, &measured_id) else {
                continue;
            };
            if s > 0.0 && r2 >= 0.8 {
                let err = if model_is_valid {
                    output_tail_slope(&model_vd, &model_id)
                        .map(|(m, _)| (m / s - 1.0).abs())
                        .filter(|e| e.is_finite())
                        .unwrap_or(1.0)
                } else {
                    1.0
                };
                errors.push(err);
            }
        }
        errors.sort_by(f64::total_cmp);
        (errors.len() >= 2)
            .then(|| percentile(&errors, 0.9))
            .flatten()
    }
}

fn curve(vd: &[f64], f: impl FnOnce(&[f64]) -> Vec<f64>) -> Vec<[f64; 2]> {
    f(vd).into_iter().zip(vd).map(|(i, &v)| [v, i]).collect()
}

fn percentile(sorted: &[f64], fraction: f64) -> Option<f64> {
    (!sorted.is_empty()).then(|| sorted[((sorted.len() - 1) as f64 * fraction).round() as usize])
}

fn longest_monotone_gain_branch(samples: &[Option<[f64; 2]>]) -> Vec<[f64; 2]> {
    let (mut best, mut run) = (Vec::new(), Vec::new());
    for sample in samples {
        let Some(point) = sample.filter(|p| p[0].is_finite() && p[1].is_finite()) else {
            run.clear();
            continue;
        };
        if run
            .last()
            .is_some_and(|previous: &[f64; 2]| point[0] <= previous[0] || point[1] < previous[1])
        {
            run.clear();
        }
        run.push(point);
        if run.len() > best.len() {
            best.clone_from(&run);
        }
    }
    best
}
