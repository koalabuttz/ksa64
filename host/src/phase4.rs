//! Deterministic native Phase 4 campaign execution and ordered artifact encoding.

use std::thread;

use ksa64_core::phase2_scenario::Phase2Scenario;
use ksa64_sim::mission::MissionError;
use ksa64_sim::phase4::aggregate::CampaignAggregate;
use ksa64_sim::phase4::campaign::{derive_run, CampaignConfig, CampaignError};
use ksa64_sim::phase4::contracts::RUN_SUMMARY_LENGTH;
use ksa64_sim::phase4::mission::run_phase4_mission;
use ksa64_sim::phase4::summary::{write_ksr4, RunSummary, SummaryError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostCampaignError {
    WorkerCount,
    Campaign(CampaignError),
    Mission { run_index: u32, error: MissionError },
    WorkerPanic,
    RunOrder,
    Summary(SummaryError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostCampaign {
    pub summaries: Vec<RunSummary>,
    pub aggregate: CampaignAggregate,
}

fn execute_indices(
    scenario: &Phase2Scenario,
    config: &CampaignConfig,
    campaign_crc32: u32,
    worker_index: usize,
    worker_count: usize,
) -> Result<Vec<RunSummary>, HostCampaignError> {
    let mut summaries = Vec::new();
    let mut index = worker_index as u32;
    while index < config.run_count {
        let run = derive_run(config, index).map_err(HostCampaignError::Campaign)?;
        let result =
            run_phase4_mission(scenario, run).map_err(|error| HostCampaignError::Mission {
                run_index: index,
                error,
            })?;
        summaries.push(RunSummary::from_result(
            scenario,
            campaign_crc32,
            run,
            result,
        ));
        index = index.saturating_add(worker_count as u32);
    }
    Ok(summaries)
}

pub fn execute_host_campaign(
    scenario: &Phase2Scenario,
    config: &CampaignConfig,
    campaign_crc32: u32,
    worker_count: usize,
) -> Result<HostCampaign, HostCampaignError> {
    config.validate().map_err(HostCampaignError::Campaign)?;
    if worker_count == 0 || worker_count > config.run_count as usize {
        return Err(HostCampaignError::WorkerCount);
    }
    let mut summaries = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for worker in 0..worker_count {
            handles.push(scope.spawn(move || {
                execute_indices(scenario, config, campaign_crc32, worker, worker_count)
            }));
        }
        let mut combined = Vec::with_capacity(config.run_count as usize);
        for handle in handles {
            let mut partial = handle
                .join()
                .map_err(|_| HostCampaignError::WorkerPanic)??;
            combined.append(&mut partial);
        }
        Ok::<_, HostCampaignError>(combined)
    })?;
    summaries.sort_unstable_by_key(|summary| summary.run_index);
    if summaries.len() != config.run_count as usize
        || summaries
            .iter()
            .enumerate()
            .any(|(index, summary)| summary.run_index != index as u32)
    {
        return Err(HostCampaignError::RunOrder);
    }
    let mut aggregate = CampaignAggregate::new();
    for summary in &summaries {
        aggregate.update(summary);
    }
    Ok(HostCampaign {
        summaries,
        aggregate,
    })
}

pub fn encode_summary_stream(campaign: &HostCampaign) -> Result<Vec<u8>, HostCampaignError> {
    let mut bytes = vec![0u8; campaign.summaries.len() * RUN_SUMMARY_LENGTH];
    for (index, summary) in campaign.summaries.iter().enumerate() {
        let start = index * RUN_SUMMARY_LENGTH;
        write_ksr4(summary, &mut bytes[start..start + RUN_SUMMARY_LENGTH])
            .map_err(HostCampaignError::Summary)?;
    }
    Ok(bytes)
}
