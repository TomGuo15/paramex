//! The metric science — a faithful Rust port of `extraction.{sweep,on_off,vth,
//! ss,hysteresis}`. Each submodule mirrors one Python module and reuses the
//! golden-validated lower layers (`fit`, `numerics`, `numpy_compat`).
//!
//! Metric implementations are grouped by extracted quantity.

pub(super) mod hysteresis;
pub(super) mod ss;
pub(super) mod sweep;
pub(super) mod vth;

#[cfg(test)]
mod tests;
