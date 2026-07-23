use ksa64_sim::phase4::campaign::CampaignError;
use ksa64_sim::phase5_campaign::{
    run_phase5_campaign_mission, write_ksr5, Phase5CampaignAggregate, Phase5CampaignConfig,
    Phase5CampaignError, Phase5RunSummary, KSR5_LENGTH,
};
use std::thread;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostPhase5CampaignError {
    Workers,
    Campaign(CampaignError),
    Run {
        index: u32,
        error: Phase5CampaignError,
    },
    Panic,
    Order,
    Summary,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostPhase5Campaign {
    pub summaries: Vec<Phase5RunSummary>,
    pub aggregate: Phase5CampaignAggregate,
}
fn execute_indices(
    c: &Phase5CampaignConfig,
    worker: usize,
    workers: usize,
) -> Result<Vec<Phase5RunSummary>, HostPhase5CampaignError> {
    let mut out = Vec::new();
    let mut i = worker as u32;
    while i < c.run_count {
        out.push(
            run_phase5_campaign_mission(c, i)
                .map_err(|error| HostPhase5CampaignError::Run { index: i, error })?,
        );
        i = i.saturating_add(workers as u32)
    }
    Ok(out)
}
pub fn execute_phase5_campaign(
    c: &Phase5CampaignConfig,
    workers: usize,
) -> Result<HostPhase5Campaign, HostPhase5CampaignError> {
    c.validate().map_err(HostPhase5CampaignError::Campaign)?;
    if workers == 0 || workers > c.run_count as usize {
        return Err(HostPhase5CampaignError::Workers);
    }
    let mut summaries = thread::scope(|scope| {
        let mut handles = Vec::new();
        for w in 0..workers {
            handles.push(scope.spawn(move || execute_indices(c, w, workers)))
        }
        let mut all = Vec::with_capacity(c.run_count as usize);
        for h in handles {
            all.append(&mut h.join().map_err(|_| HostPhase5CampaignError::Panic)??)
        }
        Ok::<_, HostPhase5CampaignError>(all)
    })?;
    summaries.sort_unstable_by_key(|s| s.run_index);
    if summaries.len() != c.run_count as usize
        || summaries
            .iter()
            .enumerate()
            .any(|(i, s)| s.run_index != i as u32)
    {
        return Err(HostPhase5CampaignError::Order);
    }
    let mut aggregate = Phase5CampaignAggregate::new();
    for s in &summaries {
        aggregate.update(s)
    }
    Ok(HostPhase5Campaign {
        summaries,
        aggregate,
    })
}
pub fn encode_ksr5_stream(c: &HostPhase5Campaign) -> Result<Vec<u8>, HostPhase5CampaignError> {
    let mut out = vec![0u8; c.summaries.len() * KSR5_LENGTH];
    for (i, s) in c.summaries.iter().enumerate() {
        write_ksr5(s, &mut out[i * KSR5_LENGTH..(i + 1) * KSR5_LENGTH])
            .map_err(|_| HostPhase5CampaignError::Summary)?
    }
    Ok(out)
}
