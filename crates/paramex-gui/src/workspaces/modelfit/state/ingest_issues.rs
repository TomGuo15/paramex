//! Persistent Model Fit failures rendered after the device/pending rows.

#[derive(Debug, Clone, PartialEq, Eq)]
struct Issue {
    id: String,
    name: String,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IssueRow<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub message: &'a str,
}

#[derive(Default)]
pub struct IngestIssues {
    next_id: u64,
    // ponytail: UI issue lists are small; use keyed storage only if profiling
    // shows these linear scans matter.
    issues: Vec<Issue>,
}

impl IngestIssues {
    pub fn record(&mut self, name: String, message: String) {
        let id = format!("ingest-error-{}", self.next_id);
        self.next_id += 1;
        self.issues.push(Issue { id, name, message });
    }

    pub fn rows(&self) -> impl Iterator<Item = IssueRow<'_>> + '_ {
        self.issues.iter().map(|issue| IssueRow {
            id: &issue.id,
            name: &issue.name,
            message: &issue.message,
        })
    }

    pub fn has_errors(&self) -> bool {
        !self.issues.is_empty()
    }

    pub fn clear(&mut self) -> usize {
        let count = self.issues.len();
        self.issues.clear();
        count
    }

    pub fn dismiss(&mut self, id: &str) -> bool {
        let previous_len = self.issues.len();
        self.issues.retain(|issue| issue.id != id);
        self.issues.len() != previous_len
    }
}
