//! Ordered, allocation-free Phase 4 campaign execution.

use ksa64_core::phase2_scenario::Phase2Scenario;

use crate::mission::MissionError;

use super::aggregate::CampaignAggregate;
use super::campaign::{derive_run, CampaignConfig, CampaignError};
use super::mission::run_phase4_mission;
use super::summary::{RunSummary, SummaryError};

pub trait CampaignSink {
    type Error;
    fn observe(&mut self, summary: &RunSummary) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignRunError<E> {
    Campaign(CampaignError),
    Mission { run_index: u32, error: MissionError },
    Summary(SummaryError),
    Sink(E),
}

pub struct NullCampaignSink;
impl CampaignSink for NullCampaignSink {
    type Error = core::convert::Infallible;
    fn observe(&mut self, _summary: &RunSummary) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub fn run_campaign<S: CampaignSink>(
    scenario: &Phase2Scenario,
    config: &CampaignConfig,
    campaign_crc32: u32,
    sink: &mut S,
) -> Result<CampaignAggregate, CampaignRunError<S::Error>> {
    config.validate().map_err(CampaignRunError::Campaign)?;
    let mut aggregate = CampaignAggregate::new();
    let mut index = 0;
    while index < config.run_count {
        let run = derive_run(config, index).map_err(CampaignRunError::Campaign)?;
        let mission =
            run_phase4_mission(scenario, run).map_err(|error| CampaignRunError::Mission {
                run_index: index,
                error,
            })?;
        let summary = RunSummary::from_result(scenario, campaign_crc32, run, mission);
        sink.observe(&summary).map_err(CampaignRunError::Sink)?;
        aggregate.update(&summary);
        index += 1;
    }
    Ok(aggregate)
}
