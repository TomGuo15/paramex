pub mod modelfit;
pub(crate) mod output_ingest;
pub mod tlm;
pub mod transfer;

/// Replace the incoming value's complete direct-match set at its earliest position.
fn upsert_match_set<T>(values: &mut Vec<T>, incoming: T, matches: impl Fn(&T, &T) -> bool) {
    let Some(index) = values.iter().position(|value| matches(value, &incoming)) else {
        values.push(incoming);
        return;
    };
    values.retain(|value| !matches(value, &incoming));
    values.insert(index, incoming);
}
