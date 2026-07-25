//! Host composition for the Phase 8.5 local-ENU world and avionics.
use ksa64_core::phase8_5_contract::{
    ActuatorCapabilityPack, AvionicsEvaluationSummary, AvionicsProfilePack,
};
use ksa64_core::phase8_mission::SpatialMissionVariation;
use ksa64_core::phase8_pack::{
    parse_spatial_mission_pack, parse_spatial_motor_pack, parse_spatial_vehicle_pack,
    parse_wind_profile_pack, SpatialMissionPack, SpatialMotorPack, SpatialVehiclePack,
    WindProfilePack,
};
use ksa64_flight::phase8_5::{LocalFlightComputer, LocalFlightEvidence};
use ksa64_interface::phase6::crc16_ccitt;
use ksa64_interface::phase8_5::{write_local_aid, Kat8Frame, LocalAidCell};
use ksa64_sim::phase8_5::{
    derive_gimbal_derivative_vehicle, evaluate_with_avionics, local_flight_config,
    reference_avionics_profile, reference_gimbal_capability, reference_monitor_capability,
    AvionicsEvaluationRequest, LocalAvionicsVariation, LocalDirectorSample, LocalWorldEndpoint,
    LocalWorldError,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalPlacement {
    HostHost,
    HostExternalFlight,
    CombinedC64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceConfiguration {
    pub vehicle: SpatialVehiclePack,
    pub motor: SpatialMotorPack,
    pub mission: SpatialMissionPack,
    pub wind: WindProfilePack,
    pub avionics: AvionicsProfilePack,
    pub capability: ActuatorCapabilityPack,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Phase85Update {
    pub epoch: u16,
    pub time_s: f64,
    pub phase: u8,
    pub events: u16,
    pub truth_position_m: [f64; 3],
    pub truth_velocity_mps: [f64; 3],
    pub onboard_position_m: [f64; 3],
    pub onboard_velocity_mps: [f64; 3],
    pub ground_position_m: [f64; 3],
    pub ground_velocity_mps: [f64; 3],
    pub attitude_vector: [i16; 3],
    pub angular_rate: [i16; 3],
    pub control_demand: [i16; 2],
    pub commanded_gimbal: [i16; 2],
    pub applied_gimbal: [i16; 2],
    pub inertial_validity: u8,
    pub aid_validity: u16,
    pub alarms: u16,
    pub flight_mode: u8,
    pub armed: bool,
    pub drogue_latched: bool,
    pub main_latched: bool,
    pub mass_kg: f64,
    pub thrust_n: f64,
    pub mach: f64,
    pub dynamic_pressure_pa: f64,
    pub angle_of_attack_deg: f64,
    pub static_margin: f64,
    pub wind_mps: [f64; 3],
    pub truth_checksum: u32,
    pub navigation_checksum: u32,
    pub flight_checksum: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Kmr8Recording {
    pub schema: String,
    pub placement: LocalPlacement,
    pub vehicle_identity: u32,
    pub avionics_identity: u32,
    pub actuator_identity: u32,
    pub updates: Vec<Phase85Update>,
    pub terminal_checksum_chains: [u32; 6],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase85RunEvidence {
    pub placement: LocalPlacement,
    pub releases: u32,
    pub summary: AvionicsEvaluationSummary,
}
#[derive(Debug)]
pub enum Phase85HostError {
    Configuration,
    World(LocalWorldError),
    Incomplete,
}
impl From<LocalWorldError> for Phase85HostError {
    fn from(value: LocalWorldError) -> Self {
        Self::World(value)
    }
}
pub trait Phase85Sink {
    fn publish(&mut self, update: &Phase85Update, frame: &Kat8Frame);
    fn finish(&mut self, _evidence: &Phase85RunEvidence) {}
}

pub fn checked_in_reference(gimbal: bool) -> Result<ReferenceConfiguration, Phase85HostError> {
    let base = parse_spatial_vehicle_pack(include_bytes!("../../phase8/examples/firestorm54.kvp8"))
        .map_err(|_| Phase85HostError::Configuration)?;
    let motor =
        parse_spatial_motor_pack(include_bytes!("../../phase8/examples/aerotech-i211w.kmp8"))
            .map_err(|_| Phase85HostError::Configuration)?;
    let mut mission =
        parse_spatial_mission_pack(include_bytes!("../../phase8/examples/firestorm-i211.kmc8"))
            .map_err(|_| Phase85HostError::Configuration)?;
    let wind = parse_wind_profile_pack(include_bytes!("../../phase8/examples/firestorm-calm.kwp8"))
        .map_err(|_| Phase85HostError::Configuration)?;
    let (vehicle, capability) = if gimbal {
        let capability = reference_gimbal_capability(0x8500_1001);
        let vehicle = derive_gimbal_derivative_vehicle(base, capability)?;
        mission.vehicle_identity = vehicle.identity;
        (vehicle, capability)
    } else {
        (base, reference_monitor_capability(base.identity))
    };
    Ok(ReferenceConfiguration {
        vehicle,
        motor,
        mission,
        wind,
        avionics: reference_avionics_profile(gimbal),
        capability,
    })
}

fn q13(value: i32) -> f64 {
    f64::from(value) / 8192.0
}
fn q19(value: i32) -> f64 {
    f64::from(value) / 524_288.0
}
fn sample(
    release: LocalDirectorSample,
    inertial: ksa64_interface::phase8_5::LocalInertialCell,
    aid: Option<LocalAidCell>,
    flight: LocalFlightEvidence,
) -> Phase85Update {
    let snapshot = release.snapshot;
    let state = snapshot.state;
    let ground_position = [
        q13(state.position.x()),
        q13(state.position.y()),
        q13(state.position.z()),
    ];
    let ground_velocity = [
        q19(state.velocity.x()),
        q19(state.velocity.y()),
        q19(state.velocity.z()),
    ];
    Phase85Update {
        epoch: release.epoch,
        time_s: f64::from(state.time.raw()) / 262_144.0,
        phase: snapshot.phase as u8,
        events: snapshot.events,
        truth_position_m: ground_position,
        truth_velocity_mps: ground_velocity,
        onboard_position_m: flight.navigation.position_q13.map(q13),
        onboard_velocity_mps: flight.navigation.velocity_q19.map(q19),
        ground_position_m: ground_position.map(|value| (value * 10.0).round() / 10.0),
        ground_velocity_mps: ground_velocity.map(|value| (value * 100.0).round() / 100.0),
        attitude_vector: inertial.platform_angle,
        angular_rate: inertial.angular_rate,
        control_demand: flight.command.control_demand,
        commanded_gimbal: flight.command.gimbal,
        applied_gimbal: release.applied_gimbal,
        inertial_validity: inertial.validity,
        aid_validity: aid.map(|value| value.validity).unwrap_or(0),
        alarms: flight.alarms,
        flight_mode: flight.status.map(|value| value.mode).unwrap_or(0),
        armed: flight.armed,
        drogue_latched: flight.drogue_latched,
        main_latched: flight.main_latched,
        mass_kg: f64::from(snapshot.mass.mass.raw()) / 2_097_152.0,
        thrust_n: f64::from(snapshot.thrust_q13) / 8192.0,
        mach: f64::from(snapshot.aero.mach_q24) / 16_777_216.0,
        dynamic_pressure_pa: f64::from(snapshot.aero.dynamic_pressure_q13) / 8192.0,
        angle_of_attack_deg: f64::from(snapshot.aero.angle_of_attack_q28) / 268_435_456.0 * 180.0
            / std::f64::consts::PI,
        static_margin: f64::from(snapshot.aero.static_margin_q24) / 16_777_216.0,
        wind_mps: snapshot
            .wind_q22
            .map(|value| f64::from(value) / 4_194_304.0),
        truth_checksum: release.truth_checksum,
        navigation_checksum: flight.navigation.checksum,
        flight_checksum: flight.flight_checksum,
    }
}
fn kat_frame(
    update: &Phase85Update,
    release: ksa64_sim::phase8_5::LocalWorldRelease,
    flight: LocalFlightEvidence,
) -> Kat8Frame {
    let mut aid_bytes = [0u8; ksa64_interface::phase8_5::LOCAL_AID_LENGTH];
    let aid_crc16 = release
        .aid
        .and_then(|value| {
            write_local_aid(&value, &mut aid_bytes).ok()?;
            Some(crc16_ccitt(&aid_bytes))
        })
        .unwrap_or(0);
    Kat8Frame {
        epoch: update.epoch,
        phase: update.phase,
        flags: u8::from(flight.safe),
        time_q18: release.director.snapshot.state.time.raw(),
        director_checksum: update.truth_checksum,
        inertial: release.inertial,
        command: flight.command,
        status: flight.status,
        aid_crc16,
        aid_validity: update.aid_validity,
        truth_altitude_q13: release.director.snapshot.state.position.z(),
        truth_velocity_q19: [
            release.director.snapshot.state.velocity.x(),
            release.director.snapshot.state.velocity.y(),
            release.director.snapshot.state.velocity.z(),
        ],
        applied_gimbal: release.director.applied_gimbal,
        events: update.events,
        deployment_feedback: release.inertial.actuator_feedback,
    }
}

pub fn run_host_host(
    gimbal: bool,
    mut sink: Option<&mut dyn Phase85Sink>,
) -> Result<Phase85RunEvidence, Phase85HostError> {
    let reference = checked_in_reference(gimbal)?;
    let config = local_flight_config(reference.avionics, reference.capability, &reference.motor)?;
    let mut world = LocalWorldEndpoint::new(
        &reference.vehicle,
        &reference.motor,
        reference.mission,
        &reference.wind,
        SpatialMissionVariation::NOMINAL,
        reference.capability,
    )?;
    let initial = world.snapshot().state;
    let q = initial.attitude;
    let mut flight = LocalFlightComputer::new(
        config,
        [
            initial.position.x(),
            initial.position.y(),
            initial.position.z(),
        ],
        [
            (q.x() >> 15) as i16,
            (q.y() >> 15) as i16,
            (q.z() >> 15) as i16,
        ],
    )
    .ok_or(Phase85HostError::Configuration)?;
    let mut releases = 0u32;
    while !world.is_complete() {
        let Some(release) = world.release()? else {
            break;
        };
        let flight_out = flight.tick(Some(release.inertial), release.aid);
        world.accept_command(flight_out.command)?;
        let update = sample(release.director, release.inertial, release.aid, flight_out);
        let frame = kat_frame(&update, release, flight_out);
        if let Some(target) = sink.as_deref_mut() {
            target.publish(&update, &frame);
        }
        releases = releases.saturating_add(1);
    }
    let summary = evaluate_with_avionics(AvionicsEvaluationRequest {
        vehicle: &reference.vehicle,
        motor: &reference.motor,
        mission: reference.mission,
        wind: &reference.wind,
        variation: SpatialMissionVariation::NOMINAL,
        variation_checksum: 0,
        avionics: reference.avionics,
        capability: reference.capability,
        uncertainty_case: LocalAvionicsVariation::NOMINAL,
    })?;
    let evidence = Phase85RunEvidence {
        placement: LocalPlacement::HostHost,
        releases,
        summary,
    };
    if let Some(target) = sink {
        target.finish(&evidence);
    }
    Ok(evidence)
}

pub struct RecordingSink {
    placement: LocalPlacement,
    vehicle_identity: u32,
    avionics_identity: u32,
    actuator_identity: u32,
    updates: Vec<Phase85Update>,
    frames: Vec<[u8; ksa64_interface::phase8_5::KAT8_FRAME_LENGTH]>,
}
impl RecordingSink {
    pub fn new(reference: ReferenceConfiguration, placement: LocalPlacement) -> Self {
        Self {
            placement,
            vehicle_identity: reference.vehicle.identity,
            avionics_identity: reference.avionics.identity,
            actuator_identity: reference.capability.identity,
            updates: Vec::new(),
            frames: Vec::new(),
        }
    }
    pub fn recording(&self, chains: [u32; 6]) -> Kmr8Recording {
        Kmr8Recording {
            schema: "ksa64.kmr8-v1".into(),
            placement: self.placement,
            vehicle_identity: self.vehicle_identity,
            avionics_identity: self.avionics_identity,
            actuator_identity: self.actuator_identity,
            updates: self.updates.clone(),
            terminal_checksum_chains: chains,
        }
    }
    pub fn kat_frames(&self) -> &[[u8; ksa64_interface::phase8_5::KAT8_FRAME_LENGTH]] {
        &self.frames
    }
}
impl Phase85Sink for RecordingSink {
    fn publish(&mut self, update: &Phase85Update, frame: &Kat8Frame) {
        self.updates.push(update.clone());
        let mut bytes = [0; ksa64_interface::phase8_5::KAT8_FRAME_LENGTH];
        if ksa64_interface::phase8_5::write_kat8_frame(frame, &mut bytes).is_ok() {
            self.frames.push(bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Count(u32);
    impl Phase85Sink for Count {
        fn publish(&mut self, _: &Phase85Update, _: &Kat8Frame) {
            self.0 += 1;
        }
    }
    #[test]
    fn passive_recording_cannot_change_physics() {
        let plain = run_host_host(false, None).unwrap();
        let mut count = Count(0);
        let observed = run_host_host(false, Some(&mut count)).unwrap();
        assert_eq!(plain.summary, observed.summary);
        assert_eq!(count.0, observed.releases);
    }
    #[test]
    fn monitor_and_gimbal_are_separately_identified() {
        let monitor = checked_in_reference(false).unwrap();
        let gimbal = checked_in_reference(true).unwrap();
        assert_ne!(monitor.vehicle.identity, gimbal.vehicle.identity);
        assert_ne!(monitor.capability.identity, gimbal.capability.identity);
    }
}
