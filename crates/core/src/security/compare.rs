//! Baseline versus current report (Purple regression, ADR 0032 §5). Pure: both reports were opened by
//! the user and nothing is stored. "No longer reported" means the scanner stopped reporting a finding,
//! not that anyone verified it fixed.

use super::finding::Finding;
use super::sarif::Report;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// In the current report only.
    pub new: Vec<Finding>,
    /// In both.
    pub still_present: Vec<Finding>,
    /// In the baseline only.
    pub no_longer_reported: Vec<Finding>,
    /// True when either report had results it did not keep (over the limit), so the lists may be wrong.
    pub incomplete: bool,
}

/// Match by fingerprint as a multiset: two identical findings in the baseline and one in the current
/// report give one "still present" and one "no longer reported". Report order is kept.
pub fn compare(baseline: &Report, current: &Report) -> Comparison {
    let mut left: HashMap<&str, usize> = HashMap::new();
    for f in &baseline.findings {
        *left.entry(f.fingerprint.as_str()).or_insert(0) += 1;
    }
    let (mut new, mut still_present) = (vec![], vec![]);
    for f in &current.findings {
        match left.get_mut(f.fingerprint.as_str()) {
            Some(n) if *n > 0 => {
                *n -= 1;
                still_present.push(f.clone());
            }
            _ => new.push(f.clone()),
        }
    }
    let mut no_longer_reported = vec![];
    for f in &baseline.findings {
        if let Some(n) = left.get_mut(f.fingerprint.as_str()) {
            if *n > 0 {
                *n -= 1;
                no_longer_reported.push(f.clone());
            }
        }
    }
    Comparison {
        new,
        still_present,
        no_longer_reported,
        incomplete: baseline.not_shown > 0 || current.not_shown > 0,
    }
}
