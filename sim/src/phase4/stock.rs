//! Stock-C64 streaming retention for Phase 4 campaigns.

use super::aggregate::CampaignAggregate;
use super::contracts::STOCK_INTERESTING_SUMMARIES;
use super::summary::{RunOutcome, RunSummary};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StockError {
    RunOrder,
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockSnapshot {
    pub aggregate: CampaignAggregate,
    pub retained: [RunSummary; STOCK_INTERESTING_SUMMARIES],
}

pub struct StockRetention {
    next_index: u32,
    aggregate: CampaignAggregate,
    baseline: Option<RunSummary>,
    worst_insertion: Option<RunSummary>,
    worst_load: Option<RunSummary>,
    worst_navigation: Option<RunSummary>,
    first_failure: Option<RunSummary>,
    lowest: [Option<RunSummary>; STOCK_INTERESTING_SUMMARIES],
}

impl StockRetention {
    pub const fn new() -> Self {
        Self {
            next_index: 0,
            aggregate: CampaignAggregate::new(),
            baseline: None,
            worst_insertion: None,
            worst_load: None,
            worst_navigation: None,
            first_failure: None,
            lowest: [None; STOCK_INTERESTING_SUMMARIES],
        }
    }

    pub fn observe(&mut self, summary: RunSummary) -> Result<(), StockError> {
        if summary.run_index != self.next_index {
            return Err(StockError::RunOrder);
        }
        self.next_index += 1;
        self.aggregate.update(&summary);
        if summary.run_index == 0 {
            self.baseline = Some(summary);
        }
        replace_if(&mut self.worst_insertion, summary, |candidate, current| {
            candidate.cutoff_radius_q12 < current.cutoff_radius_q12
                || (candidate.cutoff_radius_q12 == current.cutoff_radius_q12
                    && candidate.run_index < current.run_index)
        });
        replace_if(&mut self.worst_load, summary, |candidate, current| {
            candidate.max_dynamic_pressure_q16 > current.max_dynamic_pressure_q16
                || (candidate.max_dynamic_pressure_q16 == current.max_dynamic_pressure_q16
                    && candidate.run_index < current.run_index)
        });
        replace_if(&mut self.worst_navigation, summary, |candidate, current| {
            candidate.navigation_position_error_q12 > current.navigation_position_error_q12
                || (candidate.navigation_position_error_q12
                    == current.navigation_position_error_q12
                    && candidate.run_index < current.run_index)
        });
        if self.first_failure.is_none() && summary.outcome != RunOutcome::StableOrbit {
            self.first_failure = Some(summary);
        }
        if (summary.run_index as usize) < STOCK_INTERESTING_SUMMARIES {
            self.lowest[summary.run_index as usize] = Some(summary);
        }
        Ok(())
    }

    pub fn finish(self) -> Result<StockSnapshot, StockError> {
        let baseline = self.baseline.ok_or(StockError::Empty)?;
        let mut retained = [baseline; STOCK_INTERESTING_SUMMARIES];
        let mut count = 0usize;
        for candidate in [
            self.baseline,
            self.worst_insertion,
            self.worst_load,
            self.worst_navigation,
            self.first_failure,
        ] {
            if let Some(summary) = candidate {
                push_unique(&mut retained, &mut count, summary);
            }
        }
        for candidate in self.lowest.into_iter().flatten() {
            push_unique(&mut retained, &mut count, candidate);
        }
        if count != STOCK_INTERESTING_SUMMARIES {
            return Err(StockError::Empty);
        }
        Ok(StockSnapshot {
            aggregate: self.aggregate,
            retained,
        })
    }
}

impl Default for StockRetention {
    fn default() -> Self {
        Self::new()
    }
}

fn replace_if<F>(slot: &mut Option<RunSummary>, candidate: RunSummary, better: F)
where
    F: FnOnce(&RunSummary, &RunSummary) -> bool,
{
    match slot {
        Some(current) if !better(&candidate, current) => {}
        _ => *slot = Some(candidate),
    }
}

fn push_unique(
    retained: &mut [RunSummary; STOCK_INTERESTING_SUMMARIES],
    count: &mut usize,
    candidate: RunSummary,
) {
    if *count == retained.len()
        || retained[..*count]
            .iter()
            .any(|summary| summary.run_index == candidate.run_index)
    {
        return;
    }
    retained[*count] = candidate;
    *count += 1;
}
