//! Shared terminal ordering for typed Model Fit pending measurements.

use crate::workspaces::upsert_match_set;

pub(super) enum PendingEffect<S, P> {
    PreCommandDisplacement { ordinal: usize, pending: P },
    CurrentDetach(P),
    FreshClear { ordinal: usize, source: S },
    FreshPending { ordinal: usize, pending: P },
}

pub(super) fn apply_pending_effects<S: Clone, P>(
    pending_rows: &mut Vec<P>,
    effects: Vec<PendingEffect<S, P>>,
    source_of: impl Fn(&P) -> &S,
    same_source: impl Fn(&S, &S) -> bool,
) {
    let mut fresh = Vec::new();
    let mut displaced = Vec::new();
    let mut detached = Vec::new();
    for effect in effects {
        match effect {
            PendingEffect::PreCommandDisplacement { ordinal, pending } => {
                displaced.push((ordinal, pending));
            }
            PendingEffect::CurrentDetach(pending) => detached.push(pending),
            fresh_effect => fresh.push(fresh_effect),
        }
    }
    fresh.sort_by_key(|effect| match effect {
        // A successful import clears an older pending value. If that same
        // imported payload is displaced later, its equal-generation pending
        // effect follows the clear and therefore survives.
        PendingEffect::FreshClear { ordinal, .. } => (*ordinal, 0_u8),
        PendingEffect::FreshPending { ordinal, .. } => (*ordinal, 1_u8),
        PendingEffect::PreCommandDisplacement { .. } | PendingEffect::CurrentDetach(_) => {
            unreachable!("partitioned above")
        }
    });
    let attached_sources = fresh
        .iter()
        .filter_map(|effect| match effect {
            PendingEffect::FreshClear { source, .. } => Some(source.clone()),
            PendingEffect::FreshPending { .. } => None,
            PendingEffect::PreCommandDisplacement { .. } | PendingEffect::CurrentDetach(_) => {
                unreachable!("partitioned above")
            }
        })
        .collect::<Vec<_>>();
    for effect in fresh {
        match effect {
            PendingEffect::FreshClear { source, .. } => {
                pending_rows.retain(|pending| !same_source(source_of(pending), &source));
            }
            PendingEffect::FreshPending { pending, .. } => {
                upsert_match_set(pending_rows, pending, |old, incoming| {
                    same_source(source_of(old), source_of(incoming))
                });
            }
            PendingEffect::PreCommandDisplacement { .. } | PendingEffect::CurrentDetach(_) => {
                unreachable!("partitioned above")
            }
        }
    }
    displaced.sort_by_key(|(ordinal, _)| *ordinal);
    let newer_pending = std::mem::take(pending_rows);
    for (_, pending) in displaced {
        let superseded = attached_sources
            .iter()
            .any(|source| same_source(source, source_of(&pending)));
        let pending_is_newer = newer_pending
            .iter()
            .any(|old| same_source(source_of(old), source_of(&pending)));
        if !superseded && !pending_is_newer {
            upsert_match_set(pending_rows, pending, |old, incoming| {
                same_source(source_of(old), source_of(incoming))
            });
        }
    }
    pending_rows.extend(newer_pending);
    for pending in detached {
        let superseded = attached_sources
            .iter()
            .any(|source| same_source(source, source_of(&pending)));
        let already_pending = pending_rows
            .iter()
            .any(|old| same_source(source_of(old), source_of(&pending)));
        if !superseded && !already_pending {
            pending_rows.push(pending);
        }
    }
}
