//! Mission and operations adapters for the consolidated host application.

use crate::application::{
    authoring_error, codec_error, debug_error, io_error, json_error, require_action, write_file,
    write_json, ApplicationError, ApplicationOutcome, Ksa64Application, MissionDisplay,
    MissionPace, MissionRequest,
};
use crate::application_fixtures::{
    GNSS_LOSS_SOURCE, PHASE7_MISSION, PHASE7_MOTOR, PHASE7_VEHICLE, SAFEHOLD_SOURCE,
};
use crate::phase10::GlobalFixtureSet;
use crate::phase10_mission::{
    capture_nominal_global_mission, mission_json, write_global_mission_artifacts,
};
use crate::phase10_tui::{run_global_console, GlobalConsoleConfig, GlobalConsolePace};
use crate::phase11_authoring::{
    compile_project_source, complete_project_session, CompiledMissionProject,
};
use crate::phase11_live::{
    LiveMissionCapability, LiveMissionSession, MissionSessionPace, GNSS_LOSS_LIVE_CAPABILITY,
};
use crate::phase11_tui::run_operations_console;
use crate::phase12b_live::{FullMissionSession, FULL_GNSS_LOSS_SOURCE};
use crate::phase7::{capture_hobby_mission, telemetry_frame_count};
use crate::phase7_plot::build_stock_kph7;
use crate::phase8::{run_checked_in_phase8, run_checked_in_phase8_crosswind};
use crate::phase8_5::run_host_host;
use crate::phase8_5_tui::{run_local_console, ConsolePace, LocalConsoleConfig};
use crate::phase9_5_workbench::{
    baseline_advanced_vector, built_in_advanced_manifest, evaluate_advanced_candidate,
    AdvancedStudyId,
};
use crate::product::{ApplicationService, SupportedAction};
use crate::project_app::session_outcome;
use ksa64_core::evaluation::MetricSlot;
use ksa64_core::phase7_pack::{parse_mission_pack, parse_motor_pack, parse_vehicle_pack};
use ksa64_core::phase9_contract::SearchEngineId;
use ksa64_sim::phase10_corroboration::coast_frozen_ksa5_one_orbit;
use serde_json::json;
use std::fs;

impl Ksa64Application {
    pub fn start_full_operations_mission(
        &self,
        request: &MissionRequest,
    ) -> Result<FullMissionSession, ApplicationError> {
        let descriptor = self.experience(&request.id)?;
        require_action(descriptor, SupportedAction::MissionControl)?;
        if descriptor.service != ApplicationService::MissionOperations
            || scenario(request, "gnss-loss-full") != "gnss-loss-full"
        {
            return Err(ApplicationError::unsupported(
                "mission.full-live-session",
                format!(
                    "{} does not expose the full Phase 12B operations adapter",
                    request.id
                ),
            ));
        }
        let role = request.role.as_deref().unwrap_or("guided-operator");
        let role = match role {
            "observer" => ksa64_interface::phase11::OperationalRole::Observer,
            "guided-operator" => ksa64_interface::phase11::OperationalRole::GuidedOperator,
            "flight-controller" => ksa64_interface::phase11::OperationalRole::FlightController,
            "flight-software-engineer" => {
                ksa64_interface::phase11::OperationalRole::FlightSoftwareEngineer
            }
            "sim-director" => ksa64_interface::phase11::OperationalRole::SimDirector,
            "scripted-operator" => ksa64_interface::phase11::OperationalRole::ScriptedOperator,
            _ => {
                return Err(ApplicationError::invalid(
                    "mission.role",
                    "unsupported operational role",
                ))
            }
        };
        let mut session = FullMissionSession::new(role).map_err(|error| {
            ApplicationError::execution(
                "mission.full-live-session",
                format!("could not compile full operations session: {error:?}"),
            )
        })?;
        session.prepare().map_err(|error| {
            ApplicationError::execution(
                "mission.full-live-session",
                format!("could not prepare full operations session: {error:?}"),
            )
        })?;
        session
            .set_pace(match request.pace {
                MissionPace::Fast => MissionSessionPace::Fast,
                MissionPace::Realtime => MissionSessionPace::Realtime,
            })
            .map_err(|error| {
                ApplicationError::execution(
                    "mission.full-live-session",
                    format!("could not set full operations pace: {error:?}"),
                )
            })?;
        Ok(session)
    }

    pub fn live_mission_capability(
        &self,
        id: &str,
        scenario_name: &str,
    ) -> Result<Option<LiveMissionCapability>, ApplicationError> {
        let descriptor = self.experience(id)?;
        Ok((descriptor.service == ApplicationService::MissionOperations
            && scenario_name == "gnss-loss")
            .then_some(GNSS_LOSS_LIVE_CAPABILITY))
    }

    /// Compile and prepare a live deterministic operations session.
    ///
    /// Phase 11.5 intentionally exposes live stepping only for experiences that
    /// declare a real incremental adapter. Synchronous evaluators fail closed
    /// rather than masquerading as interactive missions.
    pub fn start_mission(
        &self,
        request: &MissionRequest,
    ) -> Result<LiveMissionSession, ApplicationError> {
        let descriptor = self.experience(&request.id)?;
        require_action(descriptor, SupportedAction::MissionControl)?;
        if descriptor.service != ApplicationService::MissionOperations
            || scenario(request, "gnss-loss") != "gnss-loss"
        {
            return Err(ApplicationError::unsupported(
                "mission.live-session",
                format!(
                    "{} does not yet expose an incremental mission-session adapter",
                    request.id
                ),
            ));
        }
        let source = operation_source(
            "gnss-loss",
            request.role.as_deref().unwrap_or("guided-operator"),
        )?;
        let project =
            compile_project_source(&source).map_err(authoring_error("mission.live-session"))?;
        self.start_project_session(project, request.pace)
    }

    pub fn start_project_session(
        &self,
        project: CompiledMissionProject,
        pace: MissionPace,
    ) -> Result<LiveMissionSession, ApplicationError> {
        let mut session = LiveMissionSession::compiled(project).map_err(|error| {
            ApplicationError::unsupported(
                "mission.live-session",
                format!("project has no live session adapter: {error:?}"),
            )
        })?;
        session.prepare().map_err(|error| {
            ApplicationError::execution(
                "mission.live-session",
                format!("could not prepare live session: {error:?}"),
            )
        })?;
        session
            .set_pace(match pace {
                MissionPace::Fast => MissionSessionPace::Fast,
                MissionPace::Realtime => MissionSessionPace::Realtime,
            })
            .map_err(|error| {
                ApplicationError::execution(
                    "mission.live-session",
                    format!("could not set session pace: {error:?}"),
                )
            })?;
        Ok(session)
    }

    pub fn run_mission(
        &self,
        request: &MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let descriptor = self.experience(&request.id)?;
        require_action(descriptor, SupportedAction::Run)?;
        match descriptor.service {
            ApplicationService::VerticalMission => self.run_vertical(request),
            ApplicationService::SpatialMission => self.run_spatial(request),
            ApplicationService::LocalAvionics => self.run_local_avionics(request),
            ApplicationService::AdvancedCanard => {
                self.run_advanced_mission(request, AdvancedStudyId::Canard)
            }
            ApplicationService::AdvancedRcs => {
                self.run_advanced_mission(request, AdvancedStudyId::Rcs)
            }
            ApplicationService::AdvancedMixed => {
                self.run_advanced_mission(request, AdvancedStudyId::Mixed)
            }
            ApplicationService::GlobalMission => self.run_global(request),
            ApplicationService::MissionOperations => self.run_operations(request, false),
            ApplicationService::SafeholdRecovery => self.run_operations(request, true),
            ApplicationService::Ksa5aOrbitCoast => self.run_ksa5a_coast(request),
            _ => Err(ApplicationError::unsupported(
                "mission.not-runnable",
                format!("`{}` is a workbench, not a mission", request.id),
            )),
        }
    }

    pub fn mission_control(
        &self,
        mut request: MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let descriptor = self.experience(&request.id)?;
        require_action(descriptor, SupportedAction::MissionControl)?;
        request.display = MissionDisplay::Tui;
        self.run_mission(&request)
    }

    fn run_vertical(
        &self,
        request: &MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        reject_tui(request, "firestorm.vertical")?;
        ensure_scenario(request, &["nominal"])?;
        let vehicle = parse_vehicle_pack(PHASE7_VEHICLE).map_err(codec_error("phase7.vehicle"))?;
        let motor = parse_motor_pack(PHASE7_MOTOR).map_err(codec_error("phase7.motor"))?;
        let mission = parse_mission_pack(PHASE7_MISSION).map_err(codec_error("phase7.mission"))?;
        let capture = capture_hobby_mission(vehicle, &motor, mission)
            .map_err(debug_error("mission.vertical"))?;
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "Firestorm vertical mission: {:?}; {} telemetry frames",
                capture.evaluation.outcome,
                telemetry_frame_count(&capture)
            ),
            json!({
                "experience": request.id,
                "scenario": "nominal",
                "outcome": format!("{:?}", capture.evaluation.outcome),
                "frames": telemetry_frame_count(&capture),
                "apogee_raw": capture.evaluation.metric(MetricSlot::ApogeeAltitude),
                "impact_velocity_raw": capture.evaluation.metric(MetricSlot::ImpactVelocity),
                "checksum": format!("0x{:08x}", capture.evaluation.source_checksums[0]),
            }),
        );
        if let Some(output) = &request.output {
            fs::create_dir_all(output).map_err(io_error("mission.output"))?;
            let telemetry = output.join("firestorm-i211.kst7");
            let summary = output.join("firestorm-i211.ksr7");
            let plot = output.join("firestorm-i211.kph7");
            write_file(&telemetry, &capture.telemetry)?;
            write_file(&summary, &capture.summary_record)?;
            let plot_bytes =
                build_stock_kph7(&capture.telemetry).map_err(debug_error("mission.plot"))?;
            write_file(&plot, &plot_bytes)?;
            outcome = outcome
                .artifact(&telemetry)
                .artifact(&summary)
                .artifact(&plot);
        }
        Ok(outcome)
    }

    fn run_spatial(
        &self,
        request: &MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        reject_tui(request, "firestorm.spatial")?;
        let scenario = scenario(request, "calm");
        let evidence = match scenario {
            "calm" => run_checked_in_phase8(),
            "crosswind5" => run_checked_in_phase8_crosswind(5),
            _ => return Err(bad_scenario(request, &["calm", "crosswind5"])),
        }
        .map_err(debug_error("mission.spatial"))?;
        let details = serde_json::to_value(&evidence).map_err(json_error)?;
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "Firestorm spatial mission `{scenario}`: {:?}",
                evidence.outcome
            ),
            details.clone(),
        );
        if let Some(output) = &request.output {
            write_json(output, &details)?;
            outcome = outcome.artifact(output);
        }
        Ok(outcome)
    }

    fn run_local_avionics(
        &self,
        request: &MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let scenario = scenario(request, "monitor");
        let gimbal = match scenario {
            "monitor" => false,
            "gimbal" => true,
            _ => return Err(bad_scenario(request, &["monitor", "gimbal"])),
        };
        let evidence = if request.display == MissionDisplay::Tui {
            run_local_console(LocalConsoleConfig {
                gimbal,
                pace: match request.pace {
                    MissionPace::Fast => ConsolePace::Fast,
                    MissionPace::Realtime => ConsolePace::Realtime,
                },
                title: format!("KSA64 // {}", request.id),
            })
            .map_err(debug_error("mission.local-console"))?
        } else {
            run_host_host(gimbal, None).map_err(debug_error("mission.local-avionics"))?
        };
        let details = json!({
            "experience": request.id,
            "scenario": scenario,
            "placement": format!("{:?}", evidence.placement),
            "outcome": format!("{:?}", evidence.summary.physical.outcome),
            "releases": evidence.releases,
            "checksum_chains": evidence.summary.checksum_chains,
        });
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "Firestorm avionics `{scenario}`: {:?}",
                evidence.summary.physical.outcome
            ),
            details.clone(),
        );
        if let Some(output) = &request.output {
            write_json(output, &details)?;
            outcome = outcome.artifact(output);
        }
        Ok(outcome)
    }

    fn run_advanced_mission(
        &self,
        request: &MissionRequest,
        study: AdvancedStudyId,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        if request.display == MissionDisplay::Tui {
            return Err(ApplicationError::unsupported(
                "mission.advanced-tui",
                "advanced-effector live Mission Control currently uses the Phase 9.5 bridge",
            )
            .with_hint(
                "run with `--display summary`, or use the documented split-endpoint launcher",
            ));
        }
        ensure_scenario(request, &["nominal"])?;
        let manifest = built_in_advanced_manifest(study, SearchEngineId::Nsga2V1);
        let vector = baseline_advanced_vector(&manifest);
        let evidence = evaluate_advanced_candidate(&manifest, &vector, study, 1)
            .map_err(debug_error("mission.advanced"))?;
        let details = json!({
            "experience": request.id,
            "study": format!("{study:?}"),
            "manifest_identity": format!("0x{:08x}", manifest.identity),
            "candidate_identity": format!("0x{:08x}", vector.identity),
            "feasible": evidence.aggregate.feasible,
            "objectives": evidence.aggregate.objectives,
            "constraints": evidence.aggregate.constraint_values,
            "case_count": evidence.cases.len(),
        });
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "{} nominal evaluation: feasible={}",
                request.id, evidence.aggregate.feasible
            ),
            details.clone(),
        )
        .identity(vector.identity);
        if let Some(output) = &request.output {
            write_json(output, &details)?;
            outcome = outcome.artifact(output);
        }
        Ok(outcome)
    }

    fn run_global(&self, request: &MissionRequest) -> Result<ApplicationOutcome, ApplicationError> {
        ensure_scenario(request, &["nominal"])?;
        let capture = if request.display == MissionDisplay::Tui {
            run_global_console(GlobalConsoleConfig {
                title: "KSA64 // KSA-G10R GLOBAL MISSION CONTROL".into(),
                pace: match request.pace {
                    MissionPace::Fast => GlobalConsolePace::Fast,
                    MissionPace::Realtime => GlobalConsolePace::Realtime,
                },
                auto_exit: false,
            })
            .map_err(debug_error("mission.global-console"))?
        } else {
            if request.pace == MissionPace::Realtime {
                return Err(ApplicationError::unsupported(
                    "mission.realtime-without-display",
                    "realtime pacing requires the live Mission Control display",
                ));
            }
            capture_nominal_global_mission(|_| {}).map_err(debug_error("mission.global"))?
        };
        let details = mission_json(&capture);
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "KSA-G10R global mission: {:?}; {} releases",
                capture.summary.common.outcome, capture.releases
            ),
            details,
        )
        .identity(ksa64_core::phase10_telemetry::global_evaluation_identity(
            &capture.summary,
        ));
        if let Some(output) = &request.output {
            write_global_mission_artifacts(&capture, output)
                .map_err(|message| ApplicationError::execution("mission.output", message))?;
            outcome = outcome.artifact(output);
        }
        Ok(outcome)
    }

    fn run_operations(
        &self,
        request: &MissionRequest,
        safehold: bool,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        let source = if safehold {
            if scenario(request, "safehold-recovery") != "safehold-recovery" {
                return Err(bad_scenario(request, &["safehold-recovery"]));
            }
            SAFEHOLD_SOURCE.to_owned()
        } else {
            operation_source(
                scenario(request, "gnss-loss"),
                request.role.as_deref().unwrap_or("guided-operator"),
            )?
        };
        let project =
            compile_project_source(&source).map_err(authoring_error("mission.operations"))?;
        let completed = if request.display == MissionDisplay::Tui {
            run_operations_console(&project).map_err(debug_error("mission.operations-console"))?
        } else {
            complete_project_session(&project, request.scripted)
                .map_err(authoring_error("mission.operations"))?
        };
        if let Some(output) = &request.output {
            write_file(output, &completed.bundle)?;
        }
        Ok(session_outcome(
            "mission.run",
            &completed,
            request.output.as_deref(),
        ))
    }

    fn run_ksa5a_coast(
        &self,
        request: &MissionRequest,
    ) -> Result<ApplicationOutcome, ApplicationError> {
        reject_tui(request, "ksa-5a.orbit-coast")?;
        ensure_scenario(request, &["nominal"])?;
        let fixtures = GlobalFixtureSet::embedded();
        let summary = coast_frozen_ksa5_one_orbit(&fixtures.earth, &fixtures.transforms)
            .map_err(debug_error("mission.ksa5a-coast"))?;
        let details = json!({
            "handoff_identity": format!("0x{:08x}", summary.handoff.identity),
            "phase5_summary_checksum": format!("0x{:08x}", summary.handoff.phase5_summary_checksum),
            "duration_q16": summary.duration_q16,
            "steps": summary.steps,
            "terminal_position_q12_km": summary.terminal_position_q12_km,
            "terminal_velocity_q24_km_s": summary.terminal_velocity_q24_km_s,
            "minimum_altitude_q12_km": summary.minimum_altitude_q12_km,
            "maximum_altitude_q12_km": summary.maximum_altitude_q12_km,
            "checksum": format!("0x{:08x}", summary.checksum),
        });
        let mut outcome = ApplicationOutcome::new(
            "mission.run",
            format!(
                "KSA-5A one-orbit corroboration: {} steps; checksum 0x{:08x}",
                summary.steps, summary.checksum
            ),
            details.clone(),
        )
        .identity(summary.checksum);
        if let Some(output) = &request.output {
            write_json(output, &details)?;
            outcome = outcome.artifact(output);
        }
        Ok(outcome)
    }
}

fn operation_source(scenario: &str, role: &str) -> Result<String, ApplicationError> {
    if scenario == "gnss-loss" && role == "guided-operator" {
        return Ok(GNSS_LOSS_SOURCE.to_owned());
    }
    if !matches!(
        scenario,
        "nominal"
            | "gnss-loss"
            | "gnss-loss-full"
            | "guidance-update"
            | "ground-blackout"
            | "invalid-operations"
    ) {
        return Err(ApplicationError::invalid(
            "mission.scenario",
            format!("unsupported operations scenario `{scenario}`"),
        ));
    }
    let role_allowed = matches!(
        role,
        "observer"
            | "guided-operator"
            | "flight-controller"
            | "flight-software-engineer"
            | "sim-director"
            | "scripted-operator"
    );
    if !role_allowed {
        return Err(ApplicationError::invalid(
            "mission.role",
            format!("unsupported operational role `{role}`"),
        ));
    }
    if scenario == "gnss-loss-full" {
        return Ok(FULL_GNSS_LOSS_SOURCE
            .replace("guided-operator", role)
            .replace(
                "\"hints\": true",
                if role == "guided-operator" {
                    "\"hints\": true"
                } else {
                    "\"hints\": false"
                },
            ));
    }
    let source = json!({
        "schema": "ksa64.phase11.mission-project.v1",
        "name": format!("KSA-G10R {} operations", scenario),
        "scenario": scenario,
        "package": "KsaG10rReferenceOpsV1",
        "role": role,
        "definition_identity": format!("0x{:08x}", operation_definition_identity(scenario)),
        "master_seed": "0x4b5341b0",
        "hints": role == "guided-operator",
        "provenance": [{
            "kind": "accepted-model",
            "source": "KSA64 frozen Phase 10 KSA-G10R evidence",
            "identity": "0x10a00001"
        }]
    });
    serde_json::to_string_pretty(&source).map_err(json_error)
}

const fn operation_definition_identity(scenario: &str) -> u32 {
    match scenario.as_bytes() {
        b"nominal" => 0x11d1_0020,
        b"gnss-loss" => 0x11d1_0011,
        b"gnss-loss-full" => 0x12b0_1001,
        b"guidance-update" => 0x11d1_0021,
        b"ground-blackout" => 0x11d1_0022,
        b"invalid-operations" => 0x11d1_0023,
        _ => 0x11d1_00ff,
    }
}

fn scenario<'a>(request: &'a MissionRequest, default: &'a str) -> &'a str {
    request.scenario.as_deref().unwrap_or(default)
}

fn ensure_scenario(request: &MissionRequest, allowed: &[&str]) -> Result<(), ApplicationError> {
    let selected = request
        .scenario
        .as_deref()
        .unwrap_or_else(|| allowed.first().copied().unwrap_or("nominal"));
    allowed
        .contains(&selected)
        .then_some(())
        .ok_or_else(|| bad_scenario(request, allowed))
}

fn bad_scenario(request: &MissionRequest, allowed: &[&str]) -> ApplicationError {
    ApplicationError::invalid(
        "mission.scenario",
        format!(
            "scenario `{}` is not supported by `{}`; expected {}",
            request.scenario.as_deref().unwrap_or(""),
            request.id,
            allowed.join(", ")
        ),
    )
}

fn reject_tui(request: &MissionRequest, id: &str) -> Result<(), ApplicationError> {
    if request.display == MissionDisplay::Tui {
        Err(ApplicationError::unsupported(
            "mission.tui-unavailable",
            format!("`{id}` currently provides summary/evidence presentation, not a live TUI"),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::DiagnosticKind;

    fn request(id: &str) -> MissionRequest {
        MissionRequest {
            id: id.into(),
            scenario: None,
            role: None,
            display: MissionDisplay::None,
            pace: MissionPace::Fast,
            scripted: true,
            output: None,
        }
    }

    #[test]
    fn flagship_uses_accepted_phase11_services() {
        let application = Ksa64Application::default();
        let outcome = application
            .run_mission(&MissionRequest {
                scenario: Some("gnss-loss".into()),
                ..request("ksa-g10r.operations")
            })
            .unwrap();
        assert_eq!(outcome.operation, "mission.run");
        assert_eq!(outcome.details["releases"], 9);
        assert!(outcome.summary.contains("completed evidence"));
    }

    #[test]
    fn operations_sources_cover_every_catalog_scenario() {
        for scenario in [
            "nominal",
            "gnss-loss",
            "guidance-update",
            "ground-blackout",
            "invalid-operations",
        ] {
            let source = operation_source(scenario, "guided-operator").unwrap();
            compile_project_source(&source).unwrap();
        }
    }

    #[test]
    fn application_starts_a_typed_live_operations_session() {
        let application = Ksa64Application::default();
        let request = MissionRequest {
            scenario: Some("gnss-loss".into()),
            display: MissionDisplay::Tui,
            scripted: false,
            ..request("ksa-g10r.operations")
        };
        let capability = application
            .live_mission_capability("ksa-g10r.operations", "gnss-loss")
            .unwrap()
            .unwrap();
        assert_eq!(capability.release_hz, 32);
        let mut session = application.start_mission(&request).unwrap();
        assert_eq!(
            session.lifecycle(),
            crate::phase11_live::MissionSessionLifecycle::Ready
        );
        session.advance_one_release().unwrap();
        assert_eq!(session.snapshot().release_epoch, 1);
    }

    #[test]
    fn synchronous_missions_cannot_masquerade_as_live_sessions() {
        let application = Ksa64Application::default();
        let error = application
            .start_mission(&request("firestorm.vertical"))
            .err()
            .unwrap();
        assert_eq!(error.diagnostic.code, "mission.live-session");
    }

    #[test]
    fn mission_workbench_mismatch_fails_cleanly() {
        let application = Ksa64Application::default();
        let error = application
            .run_mission(&request("firestorm.design"))
            .unwrap_err();
        assert_eq!(error.diagnostic.kind, DiagnosticKind::Unsupported);
    }
}
