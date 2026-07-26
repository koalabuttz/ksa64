//! Host-only product catalog for the consolidated KSA64 application.
//!
//! This metadata is deliberately noncanonical. It explains how to reach the
//! accepted evaluators and evidence without becoming another simulation
//! authority or another binary data contract.

use serde::Serialize;
use std::collections::BTreeSet;

pub const PRODUCT_CATALOG_SCHEMA: &str = "ksa64.product-catalog.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CatalogTier {
    Current,
    Historical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExperienceKind {
    Mission,
    Operations,
    DesignStudy,
    Optimization,
    Corroboration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceMaturity {
    Accepted,
    Qualified,
    Experimental,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionPlacement {
    HostWorldHostFlight,
    HostWorldC64Flight,
    C64WorldHostFlight,
    StockC64Standalone,
    ReplayOnly,
    HostOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupportedAction {
    Compile,
    Run,
    MissionControl,
    Campaign,
    Optimize,
    Replay,
    Debrief,
    TargetRerun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimingClaim {
    HostFast,
    HostRealtime,
    C64Realtime,
    ExternallyPaced,
    LongRun,
    ReplayOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationService {
    VerticalMission,
    SpatialMission,
    LocalAvionics,
    AdvancedCanard,
    AdvancedRcs,
    AdvancedMixed,
    PassiveDesignStudy,
    ControlStudy,
    AdvancedEffectorStudy,
    GlobalMission,
    MissionOperations,
    SafeholdRecovery,
    Ksa5aOrbitCoast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HardwareRequirement {
    pub stock_c64_supported: bool,
    pub reu_required: bool,
    pub ultimate_required: bool,
    pub notes: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ExperienceDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub tier: CatalogTier,
    pub kind: ExperienceKind,
    pub maturity: EvidenceMaturity,
    pub profile: &'static str,
    pub vehicle: &'static str,
    pub mission: &'static str,
    pub avionics: &'static str,
    pub package: &'static str,
    pub scenarios: &'static [&'static str],
    pub placements: &'static [ExecutionPlacement],
    pub actions: &'static [SupportedAction],
    pub timing: TimingClaim,
    pub hardware: HardwareRequirement,
    pub envelope: &'static str,
    pub provenance_phase: &'static str,
    pub limitations: &'static str,
    pub service: ApplicationService,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct TargetDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub cargo_package: &'static str,
    pub cargo_binary: &'static str,
    pub placement: ExecutionPlacement,
    pub timing: TimingClaim,
    pub stock_c64: bool,
    pub reu_required: bool,
    pub build_owner: &'static str,
    pub stored_evidence: &'static str,
    pub live_probe_owner: &'static str,
    pub notes: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HistoricalDescriptor {
    pub id: &'static str,
    pub phase: &'static str,
    pub purpose: &'static str,
    pub audit_script: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct ProductCatalog {
    pub experiences: &'static [ExperienceDescriptor],
    pub targets: &'static [TargetDescriptor],
    pub historical: &'static [HistoricalDescriptor],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogError {
    DuplicateId(&'static str),
    MissingField(&'static str),
    MissingAction(&'static str),
    InvalidPlacement(&'static str),
    InvalidHardware(&'static str),
}

const HOST_ONLY: &[ExecutionPlacement] = &[ExecutionPlacement::HostOnly];
const HOST_AND_REPLAY: &[ExecutionPlacement] = &[
    ExecutionPlacement::HostWorldHostFlight,
    ExecutionPlacement::ReplayOnly,
];
const LOCAL_PLACEMENTS: &[ExecutionPlacement] = &[
    ExecutionPlacement::HostWorldHostFlight,
    ExecutionPlacement::HostWorldC64Flight,
    ExecutionPlacement::ReplayOnly,
];
const ADVANCED_PLACEMENTS: &[ExecutionPlacement] = &[
    ExecutionPlacement::HostWorldHostFlight,
    ExecutionPlacement::HostWorldC64Flight,
    ExecutionPlacement::C64WorldHostFlight,
    ExecutionPlacement::ReplayOnly,
];
const GLOBAL_PLACEMENTS: &[ExecutionPlacement] = &[
    ExecutionPlacement::HostWorldHostFlight,
    ExecutionPlacement::HostWorldC64Flight,
    ExecutionPlacement::ReplayOnly,
];
const STOCK_AND_HOST: &[ExecutionPlacement] = &[
    ExecutionPlacement::HostWorldHostFlight,
    ExecutionPlacement::StockC64Standalone,
    ExecutionPlacement::ReplayOnly,
];

const MISSION_ACTIONS: &[SupportedAction] = &[
    SupportedAction::Run,
    SupportedAction::MissionControl,
    SupportedAction::Campaign,
    SupportedAction::Replay,
];
const OPERATIONS_ACTIONS: &[SupportedAction] = &[
    SupportedAction::Compile,
    SupportedAction::Run,
    SupportedAction::MissionControl,
    SupportedAction::Replay,
    SupportedAction::Debrief,
    SupportedAction::TargetRerun,
];
const WORKBENCH_ACTIONS: &[SupportedAction] = &[
    SupportedAction::Optimize,
    SupportedAction::Replay,
    SupportedAction::TargetRerun,
];
const CORROBORATION_ACTIONS: &[SupportedAction] = &[SupportedAction::Run, SupportedAction::Replay];

const NO_SCENARIOS: &[&str] = &[];
const VERTICAL_SCENARIOS: &[&str] = &["nominal"];
const SPATIAL_SCENARIOS: &[&str] = &["calm", "crosswind5"];
const AVIONICS_SCENARIOS: &[&str] = &["monitor", "gimbal"];
const ADVANCED_SCENARIOS: &[&str] = &["nominal"];
const OPERATIONS_SCENARIOS: &[&str] = &[
    "nominal",
    "gnss-loss",
    "guidance-update",
    "ground-blackout",
    "invalid-operations",
];
const SAFEHOLD_SCENARIOS: &[&str] = &["safehold-recovery"];

const STOCK_OPTIONAL: HardwareRequirement = HardwareRequirement {
    stock_c64_supported: true,
    reu_required: false,
    ultimate_required: false,
    notes: "stock C64 supported; REU may increase retained evidence only",
};
const HOST_REQUIRED: HardwareRequirement = HardwareRequirement {
    stock_c64_supported: false,
    reu_required: false,
    ultimate_required: false,
    notes: "production execution is host-native; selected evidence may be replayed on C64",
};

pub static EXPERIENCES: &[ExperienceDescriptor] = &[
    ExperienceDescriptor {
        id: "firestorm.vertical",
        name: "Firestorm vertical point-mass mission",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Mission,
        maturity: EvidenceMaturity::Accepted,
        profile: "VerticalPointMassV1",
        vehicle: "Firestorm 54",
        mission: "I211W ascent and dual-deploy recovery",
        avionics: "event-driven physical profile",
        package: "none",
        scenarios: VERTICAL_SCENARIOS,
        placements: STOCK_AND_HOST,
        actions: MISSION_ACTIONS,
        timing: TimingClaim::LongRun,
        hardware: STOCK_OPTIONAL,
        envelope: "accepted Phase 7 vertical profile",
        provenance_phase: "7",
        limitations: "one-dimensional; no wind or attitude dynamics",
        service: ApplicationService::VerticalMission,
    },
    ExperienceDescriptor {
        id: "firestorm.spatial",
        name: "Firestorm local-ENU six-degree-of-freedom mission",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Mission,
        maturity: EvidenceMaturity::Accepted,
        profile: "LocalEnu6DofV1",
        vehicle: "Firestorm 54",
        mission: "rail launch, weathercocking, recovery drift",
        avionics: "truth-triggered Phase 8 executor",
        package: "none",
        scenarios: SPATIAL_SCENARIOS,
        placements: HOST_AND_REPLAY,
        actions: MISSION_ACTIONS,
        timing: TimingClaim::LongRun,
        hardware: STOCK_OPTIONAL,
        envelope: "Mach <= 0.8; angle of attack <= 15 degrees",
        provenance_phase: "8",
        limitations: "full six-degree-of-freedom ends at recovery deployment",
        service: ApplicationService::SpatialMission,
    },
    ExperienceDescriptor {
        id: "firestorm.avionics",
        name: "Firestorm local flight computer",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Mission,
        maturity: EvidenceMaturity::Accepted,
        profile: "LocalEnu6DofV1",
        vehicle: "Firestorm 54 and gimbal derivative",
        mission: "truth-blind navigation, recovery, and attitude control",
        avionics: "LocalEnuRecoveryV1 / LocalEnuGimbalV1",
        package: "KLR8 local flight endpoint",
        scenarios: AVIONICS_SCENARIOS,
        placements: LOCAL_PLACEMENTS,
        actions: MISSION_ACTIONS,
        timing: TimingClaim::ExternallyPaced,
        hardware: STOCK_OPTIONAL,
        envelope: "accepted Phase 8 local aerodynamic envelope",
        provenance_phase: "8.5",
        limitations: "combined world-plus-avionics stock image does not fit",
        service: ApplicationService::LocalAvionics,
    },
    ExperienceDescriptor {
        id: "firestorm.canard",
        name: "Firestorm aerodynamic-canard control",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Mission,
        maturity: EvidenceMaturity::Accepted,
        profile: "LocalEnu6DofV1",
        vehicle: "Firestorm-C9",
        mission: "advanced effector flight",
        avionics: "advanced KLR9 wrapper",
        package: "canard-only PriorityResidualV1",
        scenarios: ADVANCED_SCENARIOS,
        placements: ADVANCED_PLACEMENTS,
        actions: MISSION_ACTIONS,
        timing: TimingClaim::ExternallyPaced,
        hardware: STOCK_OPTIONAL,
        envelope: "Mach <= 0.8; vehicle/local incidence <= 15 degrees",
        provenance_phase: "9.5",
        limitations:
            "assumption-backed canard coefficients; crosswind and fault-matrix evidence remain available through the historical Phase 9.5 tools",
        service: ApplicationService::AdvancedCanard,
    },
    ExperienceDescriptor {
        id: "firestorm.rcs",
        name: "Firestorm cold-gas RCS control",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Mission,
        maturity: EvidenceMaturity::Accepted,
        profile: "LocalEnu6DofV1",
        vehicle: "Firestorm-R9",
        mission: "advanced effector flight",
        avionics: "advanced KLR9 wrapper",
        package: "RCS-only PriorityResidualV1",
        scenarios: ADVANCED_SCENARIOS,
        placements: ADVANCED_PLACEMENTS,
        actions: MISSION_ACTIONS,
        timing: TimingClaim::ExternallyPaced,
        hardware: STOCK_OPTIONAL,
        envelope: "compiled blowdown supply and protected reserve",
        provenance_phase: "9.5",
        limitations:
            "cold-gas source and installation are fictional; crosswind and fault-matrix evidence remain available through the historical Phase 9.5 tools",
        service: ApplicationService::AdvancedRcs,
    },
    ExperienceDescriptor {
        id: "firestorm.mixed",
        name: "Firestorm mixed gimbal, canard, and RCS control",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Mission,
        maturity: EvidenceMaturity::Accepted,
        profile: "LocalEnu6DofV1",
        vehicle: "Firestorm-M9",
        mission: "mixed-authority advanced flight",
        avionics: "advanced KLR9 wrapper",
        package: "gimbal/canard/RCS PriorityResidualV1",
        scenarios: ADVANCED_SCENARIOS,
        placements: ADVANCED_PLACEMENTS,
        actions: MISSION_ACTIONS,
        timing: TimingClaim::ExternallyPaced,
        hardware: STOCK_OPTIONAL,
        envelope: "accepted Phase 9.5 combined-authority envelope",
        provenance_phase: "9.5",
        limitations:
            "no deliberate translational RCS guidance; crosswind and fault-matrix evidence remain available through the historical Phase 9.5 tools",
        service: ApplicationService::AdvancedMixed,
    },
    ExperienceDescriptor {
        id: "firestorm.design",
        name: "Firestorm passive vehicle and recovery design",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Optimization,
        maturity: EvidenceMaturity::Accepted,
        profile: "LocalEnu6DofV1",
        vehicle: "Firestorm research derivative",
        mission: "robust passive design study",
        avionics: "monitor-only",
        package: "Phase 9 Study A",
        scenarios: NO_SCENARIOS,
        placements: HOST_ONLY,
        actions: WORKBENCH_ACTIONS,
        timing: TimingClaim::HostFast,
        hardware: HOST_REQUIRED,
        envelope: "conservative accepted geometry range",
        provenance_phase: "9",
        limitations: "production optimization is host-only",
        service: ApplicationService::PassiveDesignStudy,
    },
    ExperienceDescriptor {
        id: "firestorm.control",
        name: "Firestorm gimbal controller design",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Optimization,
        maturity: EvidenceMaturity::Accepted,
        profile: "LocalEnu6DofV1",
        vehicle: "Firestorm gimbal derivative",
        mission: "robust controller and actuator study",
        avionics: "LocalEnuGimbalV1",
        package: "Phase 9 Study B",
        scenarios: NO_SCENARIOS,
        placements: HOST_ONLY,
        actions: WORKBENCH_ACTIONS,
        timing: TimingClaim::HostFast,
        hardware: HOST_REQUIRED,
        envelope: "accepted gimbal authority and controller ranges",
        provenance_phase: "9",
        limitations: "production optimization is host-only",
        service: ApplicationService::ControlStudy,
    },
    ExperienceDescriptor {
        id: "firestorm.effectors",
        name: "Firestorm advanced-effector workbench",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Optimization,
        maturity: EvidenceMaturity::Accepted,
        profile: "LocalEnu6DofV1",
        vehicle: "Firestorm-C9/R9/M9",
        mission: "robust canard, RCS, and mixed-effector studies",
        avionics: "advanced KLR9 wrapper",
        package: "Phase 9.5 workbench",
        scenarios: NO_SCENARIOS,
        placements: HOST_ONLY,
        actions: WORKBENCH_ACTIONS,
        timing: TimingClaim::HostFast,
        hardware: HOST_REQUIRED,
        envelope: "accepted Phase 9.5 effector envelopes",
        provenance_phase: "9.5",
        limitations: "KSA-X1 evidence remains experimental",
        service: ApplicationService::AdvancedEffectorStudy,
    },
    ExperienceDescriptor {
        id: "ksa-g10r.global",
        name: "KSA-G10R global Earth mission",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Mission,
        maturity: EvidenceMaturity::Accepted,
        profile: "GlobalEcef6DofV1",
        vehicle: "KSA-G10R",
        mission: "ENU/ECEF/GCRF ascent, coast, entry, and recovery",
        avionics: "GlobalFlightComputer",
        package: "KLR10 global flight endpoint",
        scenarios: VERTICAL_SCENARIOS,
        placements: GLOBAL_PLACEMENTS,
        actions: MISSION_ACTIONS,
        timing: TimingClaim::ExternallyPaced,
        hardware: STOCK_OPTIONAL,
        envelope: "accepted research-scale global profile",
        provenance_phase: "10",
        limitations: "fictional vehicle and assumption-backed high-speed aerodynamics",
        service: ApplicationService::GlobalMission,
    },
    ExperienceDescriptor {
        id: "ksa-g10r.operations",
        name: "KSA-G10R mission operations",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Operations,
        maturity: EvidenceMaturity::Accepted,
        profile: "GlobalEcef6DofV1",
        vehicle: "KSA-G10R",
        mission: "programmable flight and deterministic ground operations",
        avionics: "KsaG10rReferenceOpsV1",
        package: "KFS11/KMP11 operations package",
        scenarios: OPERATIONS_SCENARIOS,
        placements: GLOBAL_PLACEMENTS,
        actions: OPERATIONS_ACTIONS,
        timing: TimingClaim::ExternallyPaced,
        hardware: STOCK_OPTIONAL,
        envelope: "accepted Phase 10 world and Phase 11 operations contract",
        provenance_phase: "11",
        limitations: "stock reference package uses banked RAM and is not realtime",
        service: ApplicationService::MissionOperations,
    },
    ExperienceDescriptor {
        id: "ksa-g10r.safehold",
        name: "KSA-G10R safehold and recovery package",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Operations,
        maturity: EvidenceMaturity::Qualified,
        profile: "GlobalEcef6DofV1",
        vehicle: "KSA-G10R bounded coast/entry fixture",
        mission: "safehold, entry, and recovery",
        avionics: "SafeholdRecoveryV1",
        package: "KFS11 limited backup package",
        scenarios: SAFEHOLD_SCENARIOS,
        placements: STOCK_AND_HOST,
        actions: OPERATIONS_ACTIONS,
        timing: TimingClaim::ExternallyPaced,
        hardware: STOCK_OPTIONAL,
        envelope: "declared ECI coast, ECEF entry, and local recovery only",
        provenance_phase: "11",
        limitations: "not dissimilar hardware redundancy and rejects launch/ascent",
        service: ApplicationService::SafeholdRecovery,
    },
    ExperienceDescriptor {
        id: "ksa-5a.orbit-coast",
        name: "KSA-5A global one-orbit coast corroboration",
        tier: CatalogTier::Current,
        kind: ExperienceKind::Corroboration,
        maturity: EvidenceMaturity::Qualified,
        profile: "GlobalEcef6DofV1",
        vehicle: "KSA-5A frozen insertion handoff",
        mission: "one approximately 200 km orbit",
        avionics: "none",
        package: "frozen handoff fixture",
        scenarios: VERTICAL_SCENARIOS,
        placements: HOST_AND_REPLAY,
        actions: CORROBORATION_ACTIONS,
        timing: TimingClaim::HostFast,
        hardware: HOST_REQUIRED,
        envelope: "bounded exoatmospheric coast",
        provenance_phase: "10",
        limitations: "does not run powered KSA-5A under the global profile",
        service: ApplicationService::Ksa5aOrbitCoast,
    },
];

pub static TARGETS: &[TargetDescriptor] = &[
    TargetDescriptor {
        id: "c64.firestorm.vertical",
        name: "Stock-C64 Firestorm vertical mission",
        cargo_package: "ksa64-core",
        cargo_binary: "ksa64-phase7-full-c64",
        placement: ExecutionPlacement::StockC64Standalone,
        timing: TimingClaim::LongRun,
        stock_c64: true,
        reu_required: false,
        build_owner: "phase7/complete.ps1",
        stored_evidence: "phase7/COMPLETION.md",
        live_probe_owner: "phase7/complete.ps1",
        notes: "complete accepted target mission takes about 17.72 PAL CPU minutes",
    },
    TargetDescriptor {
        id: "c64.firestorm.spatial-replay",
        name: "Stock-C64 Firestorm spatial replay",
        cargo_package: "ksa64-core",
        cargo_binary: "ksa64-phase8-replay-c64",
        placement: ExecutionPlacement::ReplayOnly,
        timing: TimingClaim::ReplayOnly,
        stock_c64: true,
        reu_required: false,
        build_owner: "phase8/complete.ps1",
        stored_evidence: "phase8/COMPLETION.md",
        live_probe_owner: "phase8/complete.ps1",
        notes: "full world mission remains a long-run target",
    },
    TargetDescriptor {
        id: "c64.firestorm.advanced-flight",
        name: "Stock-C64 advanced Firestorm flight endpoint",
        cargo_package: "ksa64-sim",
        cargo_binary: "ksa64-phase9-5-finalist-flight-endpoint-c64",
        placement: ExecutionPlacement::HostWorldC64Flight,
        timing: TimingClaim::ExternallyPaced,
        stock_c64: true,
        reu_required: false,
        build_owner: "phase9_5/complete.ps1",
        stored_evidence: "phase9_5/COMPLETION.md",
        live_probe_owner: "phase9_5/complete.ps1",
        notes: "host owns the world; selected finalist bootstrap configures the endpoint",
    },
    TargetDescriptor {
        id: "c64.ksa-g10r.global-flight",
        name: "Stock-C64 KSA-G10R global flight endpoint",
        cargo_package: "ksa64-sim",
        cargo_binary: "ksa64-phase10-flight-endpoint-c64",
        placement: ExecutionPlacement::HostWorldC64Flight,
        timing: TimingClaim::ExternallyPaced,
        stock_c64: true,
        reu_required: false,
        build_owner: "phase10/complete.ps1",
        stored_evidence: "phase10/completion-audit.json",
        live_probe_owner: "phase10/complete.ps1",
        notes: "finite release and frame-transition probes are accepted; no realtime claim",
    },
    TargetDescriptor {
        id: "c64.ksa-g10r.safehold",
        name: "Stock-C64 SafeholdRecoveryV1 endpoint",
        cargo_package: "ksa64-sim",
        cargo_binary: "ksa64-phase11-safehold-endpoint-c64",
        placement: ExecutionPlacement::StockC64Standalone,
        timing: TimingClaim::ExternallyPaced,
        stock_c64: true,
        reu_required: false,
        build_owner: "phase11/complete.ps1",
        stored_evidence: "phase11/evidence/safehold-target-layout-v1.json",
        live_probe_owner: "phase11/complete.ps1",
        notes: "flat stock image; bounded coast, entry, and recovery package",
    },
    TargetDescriptor {
        id: "c64.ksa-g10r.reference-ops",
        name: "Banked stock-C64 KSA-G10R reference operations",
        cargo_package: "ksa64-sim",
        cargo_binary: "ksa64-phase11-reference-ops-endpoint-c64",
        placement: ExecutionPlacement::HostWorldC64Flight,
        timing: TimingClaim::ExternallyPaced,
        stock_c64: true,
        reu_required: false,
        build_owner: "phase11/c64-banked/build.ps1",
        stored_evidence: "phase11/evidence/reference-ops-banked-vice-v1.json",
        live_probe_owner: "phase11/complete.ps1",
        notes: "banked RAM stopgap; interrupts disabled after entry; no realtime claim",
    },
    TargetDescriptor {
        id: "c64.ksa-g10r.global-replay",
        name: "Stock-C64 KSA-G10R global replay",
        cargo_package: "ksa64-core",
        cargo_binary: "ksa64-phase10-replay-c64",
        placement: ExecutionPlacement::ReplayOnly,
        timing: TimingClaim::ReplayOnly,
        stock_c64: true,
        reu_required: false,
        build_owner: "phase10/complete.ps1",
        stored_evidence: "phase10/completion-audit.json",
        live_probe_owner: "phase10/complete.ps1",
        notes: "passive KPH10/KSR10 evidence browser",
    },
];

pub static HISTORICAL: &[HistoricalDescriptor] = &[
    HistoricalDescriptor {
        id: "phase0",
        phase: "0",
        purpose: "toolchain and numeric feasibility",
        audit_script: "phase0/check.ps1",
    },
    HistoricalDescriptor {
        id: "phase1",
        phase: "1",
        purpose: "vertical laboratory",
        audit_script: "phase1/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase2",
        phase: "2",
        purpose: "planar orbital ascent",
        audit_script: "phase2/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase3",
        phase: "3",
        purpose: "closed-loop avionics",
        audit_script: "phase3/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase4",
        phase: "4",
        purpose: "adaptive statistical analysis",
        audit_script: "phase11_5/audits/phase4-stored.ps1",
    },
    HistoricalDescriptor {
        id: "phase5",
        phase: "5",
        purpose: "three-dimensional dynamics",
        audit_script: "phase5/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase6",
        phase: "6",
        purpose: "Commodore-in-the-loop execution",
        audit_script: "phase6/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase7",
        phase: "7",
        purpose: "multi-profile vertical evaluation",
        audit_script: "phase7/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase8",
        phase: "8",
        purpose: "local-ENU spatial flight",
        audit_script: "phase8/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase8.5",
        phase: "8.5",
        purpose: "unified local avionics",
        audit_script: "phase8_5/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase9",
        phase: "9",
        purpose: "deterministic optimization",
        audit_script: "phase9/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase9.5",
        phase: "9.5",
        purpose: "advanced effectors",
        audit_script: "phase9_5/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase10",
        phase: "10",
        purpose: "global Earth flight",
        audit_script: "phase10/complete.ps1",
    },
    HistoricalDescriptor {
        id: "phase11",
        phase: "11",
        purpose: "mission operations and programmable flight",
        audit_script: "phase11/complete.ps1",
    },
];

impl ProductCatalog {
    pub const fn accepted() -> Self {
        Self {
            experiences: EXPERIENCES,
            targets: TARGETS,
            historical: HISTORICAL,
        }
    }

    pub fn experience(&self, id: &str) -> Option<&'static ExperienceDescriptor> {
        self.experiences.iter().find(|item| item.id == id)
    }

    pub fn target(&self, id: &str) -> Option<&'static TargetDescriptor> {
        self.targets.iter().find(|item| item.id == id)
    }

    pub fn historical(&self, id: &str) -> Option<&'static HistoricalDescriptor> {
        self.historical
            .iter()
            .find(|item| item.id == id || item.phase == id)
    }

    pub fn validate(&self) -> Result<(), CatalogError> {
        let mut ids = BTreeSet::new();
        for item in self.experiences {
            if item.id.is_empty()
                || item.name.is_empty()
                || item.profile.is_empty()
                || item.provenance_phase.is_empty()
            {
                return Err(CatalogError::MissingField(item.id));
            }
            if !ids.insert(item.id) {
                return Err(CatalogError::DuplicateId(item.id));
            }
            if item.actions.is_empty() {
                return Err(CatalogError::MissingAction(item.id));
            }
            if item.placements.is_empty() {
                return Err(CatalogError::InvalidPlacement(item.id));
            }
            if item.hardware.reu_required && item.hardware.stock_c64_supported {
                return Err(CatalogError::InvalidHardware(item.id));
            }
        }
        for item in self.targets {
            if item.id.is_empty() || item.cargo_package.is_empty() || item.cargo_binary.is_empty() {
                return Err(CatalogError::MissingField(item.id));
            }
            if !ids.insert(item.id) {
                return Err(CatalogError::DuplicateId(item.id));
            }
            if item.reu_required && item.stock_c64 {
                return Err(CatalogError::InvalidHardware(item.id));
            }
        }
        for item in self.historical {
            if item.id.is_empty() || item.phase.is_empty() || item.audit_script.is_empty() {
                return Err(CatalogError::MissingField(item.id));
            }
            if !ids.insert(item.id) {
                return Err(CatalogError::DuplicateId(item.id));
            }
        }
        Ok(())
    }

    pub fn json(&self, include_historical: bool) -> serde_json::Value {
        let mut experiences = self.experiences.iter().collect::<Vec<_>>();
        experiences.sort_by_key(|item| item.id);
        let mut targets = self.targets.iter().collect::<Vec<_>>();
        targets.sort_by_key(|item| item.id);
        let mut historical = self.historical.iter().collect::<Vec<_>>();
        historical.sort_by_key(|item| item.id);
        serde_json::json!({
            "schema": PRODUCT_CATALOG_SCHEMA,
            "experiences": experiences,
            "targets": targets,
            "historical": include_historical.then_some(historical),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_catalog_is_complete_and_unique() {
        let catalog = ProductCatalog::accepted();
        assert_eq!(catalog.validate(), Ok(()));
        for id in [
            "firestorm.vertical",
            "firestorm.spatial",
            "firestorm.avionics",
            "firestorm.canard",
            "firestorm.rcs",
            "firestorm.mixed",
            "firestorm.design",
            "firestorm.control",
            "firestorm.effectors",
            "ksa-g10r.global",
            "ksa-g10r.operations",
            "ksa-g10r.safehold",
            "ksa-5a.orbit-coast",
        ] {
            assert_eq!(catalog.experience(id).map(|item| item.id), Some(id));
        }
    }

    #[test]
    fn catalog_json_is_stable_and_historical_is_opt_in() {
        let catalog = ProductCatalog::accepted();
        let current = serde_json::to_string_pretty(&catalog.json(false)).unwrap();
        let repeated = serde_json::to_string_pretty(&catalog.json(false)).unwrap();
        assert_eq!(current, repeated);
        assert!(current.contains(PRODUCT_CATALOG_SCHEMA));
        assert!(!current.contains("\"phase11\""));
        let complete = serde_json::to_string_pretty(&catalog.json(true)).unwrap();
        assert!(complete.contains("\"phase11\""));
    }

    #[test]
    fn checked_in_catalog_snapshot_is_exact() {
        let generated = serde_json::to_vec_pretty(&ProductCatalog::accepted().json(true)).unwrap();
        assert_eq!(
            generated.as_slice(),
            include_bytes!("../../phase11_5/product-catalog-v1.json")
        );
    }
}
