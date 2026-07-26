//! Campaign adapters for the consolidated host application.

use crate::application::{
    codec_error, debug_error, io_error, require_action, write_file, ApplicationError,
    ApplicationOutcome, CampaignRequest, Ksa64Application,
};
use crate::application_fixtures::{
    PHASE7_MISSION, PHASE7_MOTOR, PHASE7_VEHICLE, PHASE8_MISSION, PHASE8_MOTOR, PHASE8_VEHICLE,
    PHASE8_WIND,
};
use crate::phase10::{encode_kra10, run_global_campaign, validate_kra10, GlobalFixtureSet};
use crate::phase7_campaign::{encode_kra7, run_hobby_campaign};
use crate::phase8_5_campaign::{encode_phase85_campaign, run_phase85_campaign};
use crate::phase8_campaign::{encode_kra8, run_spatial_campaign};
use crate::phase9_5_workbench::{run_advanced_campaign, AdvancedStudyId};
use crate::product::{ApplicationService, SupportedAction};
use ksa64_core::phase7_format::KSC7_LENGTH;
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};
use ksa64_core::phase8_format::KSC8_LENGTH;
use ksa64_core::phase8_pack::{
    parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
    parse_wind_profile_pack,
};
use ksa64_interface::crc32_ieee;
use ksa64_sim::phase7_campaign::{encode_ksc7, HobbyCampaignConfig, HobbyDesignVector};
use ksa64_sim::phase8_campaign::{encode_ksc8, SpatialCampaignConfig, SPATIAL_REFERENCE_SEED};
use serde_json::json;
use std::fs;

impl Ksa64Application {
    pub fn run_campaign(
        &self,
        request: &CampaignRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        if request.runs == 0 || request.workers == 0 {
            return Err(ApplicationError::invalid(
                "campaign.invalid-budget",
                "campaign runs and workers must both be nonzero",
            ));
        }
        let descriptor = self.experience(&request.id)?;
        require_action(descriptor, SupportedAction::Campaign)?;
        fs::create_dir_all(&request.output).map_err(io_error("campaign.output"))?;
        match descriptor.service {
            ApplicationService::VerticalMission => self.campaign_vertical(request),
            ApplicationService::SpatialMission => self.campaign_spatial(request),
            ApplicationService::LocalAvionics => self.campaign_avionics(request),
            ApplicationService::AdvancedCanard => {
                self.campaign_advanced(request, AdvancedStudyId::Canard)
            }
            ApplicationService::AdvancedRcs => {
                self.campaign_advanced(request, AdvancedStudyId::Rcs)
            }
            ApplicationService::AdvancedMixed => {
                self.campaign_advanced(request, AdvancedStudyId::Mixed)
            }
            ApplicationService::GlobalMission => self.campaign_global(request),
            _ => Err(ApplicationError::unsupported(
                "campaign.unsupported",
                format!("`{}` has no campaign adapter", request.id),
            )),
        }
    }

    fn campaign_vertical(
        &self,
        request: &CampaignRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let vehicle = parse_vehicle_pack(PHASE7_VEHICLE).map_err(codec_error("phase7.vehicle"))?;
        let motor = parse_motor_pack(PHASE7_MOTOR).map_err(codec_error("phase7.motor"))?;
        let mission = parse_mission_pack(PHASE7_MISSION).map_err(codec_error("phase7.mission"))?;
        let config = HobbyCampaignConfig {
            master_seed: 0x4b53_4137,
            run_count: request.runs,
        };
        let campaign = run_hobby_campaign(
            vehicle,
            motor,
            mission,
            HobbyDesignVector::NOMINAL,
            config,
            request.workers,
        );
        let mut ksc = [0; KSC7_LENGTH];
        encode_ksc7(config, &mut ksc).map_err(debug_error("campaign.ksc7"))?;
        let ksc_path = request
            .output
            .join(format!("campaign-{}.ksc7", request.runs));
        let kra_path = request
            .output
            .join(format!("campaign-{}.kra7", request.runs));
        write_file(&ksc_path, &ksc)?;
        let archive = encode_kra7(&campaign);
        write_file(&kra_path, &archive)?;
        Ok(ApplicationOutcome::new(
            "campaign.run",
            format!("completed {} Firestorm vertical runs", request.runs),
            json!({
                "runs": request.runs,
                "workers": request.workers,
                "aggregate": format!("{:?}", campaign.aggregate),
                "archive_crc32": format!("0x{:08x}", crc32_ieee(&archive)),
            }),
        )
        .artifact(&ksc_path)
        .artifact(&kra_path))
    }

    fn campaign_spatial(
        &self,
        request: &CampaignRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let vehicle =
            parse_spatial_vehicle_pack(PHASE8_VEHICLE).map_err(codec_error("phase8.vehicle"))?;
        let motor = parse_spatial_motor_pack(PHASE8_MOTOR).map_err(codec_error("phase8.motor"))?;
        let mission =
            parse_spatial_mission_pack(PHASE8_MISSION).map_err(codec_error("phase8.mission"))?;
        let wind = parse_wind_profile_pack(PHASE8_WIND).map_err(codec_error("phase8.wind"))?;
        let run_count = u16::try_from(request.runs).map_err(|_| {
            ApplicationError::invalid("campaign.run-count", "spatial run count exceeds u16")
        })?;
        let config = SpatialCampaignConfig {
            master_seed: SPATIAL_REFERENCE_SEED,
            run_count: u32::from(run_count),
        };
        let campaign = run_spatial_campaign(vehicle, motor, mission, wind, config, request.workers);
        let mut ksc = [0; KSC8_LENGTH];
        encode_ksc8(config, &mut ksc).map_err(debug_error("campaign.ksc8"))?;
        let ksc_path = request
            .output
            .join(format!("campaign-{}.ksc8", request.runs));
        let kra_path = request
            .output
            .join(format!("campaign-{}.kra8", request.runs));
        let archive = encode_kra8(&campaign);
        write_file(&ksc_path, &ksc)?;
        write_file(&kra_path, &archive)?;
        Ok(ApplicationOutcome::new(
            "campaign.run",
            format!("completed {} Firestorm spatial runs", request.runs),
            json!({
                "runs": request.runs,
                "workers": request.workers,
                "aggregate": format!("{:?}", campaign.aggregate),
                "archive_crc32": format!("0x{:08x}", crc32_ieee(&archive)),
            }),
        )
        .artifact(&ksc_path)
        .artifact(&kra_path))
    }

    fn campaign_avionics(
        &self,
        request: &CampaignRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        if request.runs != 64 {
            return Err(ApplicationError::invalid(
                "campaign.fixed-run-count",
                "firestorm.avionics uses the frozen 64-run campaign",
            ));
        }
        let result =
            run_phase85_campaign(request.workers).map_err(debug_error("campaign.avionics"))?;
        let archive = encode_phase85_campaign(&result);
        let path = request.output.join("campaign-64.kas8");
        write_file(&path, &archive)?;
        Ok(ApplicationOutcome::new(
            "campaign.run",
            "completed frozen 64-run Firestorm avionics campaign",
            json!({
                "workers": request.workers,
                "records_crc32": format!("0x{:08x}", result.aggregate.records_crc32),
                "completed": result.aggregate.completed,
                "alarmed": result.aggregate.alarmed,
            }),
        )
        .artifact(&path))
    }

    fn campaign_advanced(
        &self,
        request: &CampaignRequest,
        study: AdvancedStudyId,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        if request.runs != 64 {
            return Err(ApplicationError::invalid(
                "campaign.fixed-run-count",
                "advanced-effector studies use the frozen 64-run campaign",
            ));
        }
        let result = run_advanced_campaign(study, request.workers)
            .map_err(debug_error("campaign.advanced"))?;
        let mut bytes = Vec::with_capacity(result.config.len() + result.records.len() * 512);
        bytes.extend_from_slice(&result.config);
        for record in &result.records {
            bytes.extend_from_slice(record);
        }
        let path = request
            .output
            .join(format!("{}-64.ksc9-kas9", advanced_study_name(study)));
        write_file(&path, &bytes)?;
        Ok(ApplicationOutcome::new(
            "campaign.run",
            format!(
                "completed frozen 64-run {} campaign",
                advanced_study_name(study)
            ),
            json!({
                "workers": request.workers,
                "records": result.records.len(),
                "crc32": format!("0x{:08x}", result.crc32),
            }),
        )
        .artifact(&path))
    }

    fn campaign_global(
        &self,
        request: &CampaignRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let runs = u16::try_from(request.runs).map_err(|_| {
            ApplicationError::invalid("campaign.run-count", "global run count exceeds u16")
        })?;
        let fixtures = GlobalFixtureSet::embedded();
        let result = run_global_campaign(&fixtures, runs, request.workers)
            .map_err(debug_error("campaign.global"))?;
        let archive = encode_kra10(&result).map_err(debug_error("campaign.kra10"))?;
        let verified = validate_kra10(&archive).map_err(debug_error("campaign.kra10"))?;
        if verified != result.aggregate {
            return Err(ApplicationError::integrity(
                "campaign.archive-mismatch",
                "global archive aggregate changed during validation",
            ));
        }
        let stem = format!("ksa-g10r-{}", request.runs);
        let archive_path = request.output.join(format!("{stem}.kra10"));
        let config_path = request.output.join(format!("{stem}.ksc10"));
        let mut config = [0; ksa64_core::phase10_telemetry::KSC10_LENGTH];
        result
            .config
            .encode(&mut config)
            .map_err(debug_error("campaign.ksc10"))?;
        write_file(&archive_path, &archive)?;
        write_file(&config_path, &config)?;
        Ok(ApplicationOutcome::new(
            "campaign.run",
            format!("completed {} KSA-G10R global runs", request.runs),
            json!({
                "runs": request.runs,
                "workers": request.workers,
                "summaries_crc32": format!("0x{:08x}", result.aggregate.summaries_crc32),
                "ground_contacts": result.aggregate.ground_contacts,
                "physical_recoveries": result.aggregate.physical_recoveries,
                "numeric_frame_time_faults": result.aggregate.numeric_frame_time_faults,
            }),
        )
        .artifact(&config_path)
        .artifact(&archive_path))
    }
}

fn advanced_study_name(study: AdvancedStudyId) -> &'static str {
    match study {
        AdvancedStudyId::Canard => "canard",
        AdvancedStudyId::Rcs => "rcs",
        AdvancedStudyId::Mixed => "mixed",
        AdvancedStudyId::Research => "research",
    }
}
