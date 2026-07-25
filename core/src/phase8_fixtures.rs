//! Generated, source-bound Phase 8 constants for stock-C64 builds.
use crate::phase8_format::{KMP8_MAX_KNOTS, KVP8_MAX_AERO_KNOTS, KWP8_MAX_WIND_KNOTS};
use crate::phase8_numeric::{
    SpatialAngle, SpatialArea, SpatialCoefficient, SpatialInertia, SpatialMass, SpatialMomentArm,
    SpatialPosition, SpatialTime, SpatialVelocity, SpatialWind,
};
use crate::phase8_pack::{
    AeroKnot, SpatialMissionPack, SpatialMotorKnot, SpatialMotorPack, SpatialVehiclePack, WindKnot,
    WindProfilePack,
};
const fn vehicle() -> SpatialVehiclePack {
    let mut knots = [AeroKnot::ZERO; KVP8_MAX_AERO_KNOTS];
    knots[0] = AeroKnot {
        mach: SpatialCoefficient::from_raw(0),
        axial_cd: SpatialCoefficient::from_raw(21139292),
        cp_from_nose: SpatialMomentArm::from_raw(420615887),
        normal_force_slope: SpatialCoefficient::from_raw(616404663),
    };
    knots[1] = AeroKnot {
        mach: SpatialCoefficient::from_raw(6710886),
        axial_cd: SpatialCoefficient::from_raw(21810381),
        cp_from_nose: SpatialMomentArm::from_raw(420615887),
        normal_force_slope: SpatialCoefficient::from_raw(616404663),
    };
    knots[2] = AeroKnot {
        mach: SpatialCoefficient::from_raw(13421773),
        axial_cd: SpatialCoefficient::from_raw(23152558),
        cp_from_nose: SpatialMomentArm::from_raw(420615887),
        normal_force_slope: SpatialCoefficient::from_raw(616404663),
    };
    SpatialVehiclePack {
        identity: 3569682864,
        dry_mass: SpatialMass::from_raw(4429268),
        length: SpatialPosition::from_raw(15502),
        diameter: SpatialPosition::from_raw(472),
        reference_area: SpatialArea::from_raw(1401777),
        dry_cg_from_nose: SpatialMomentArm::from_raw(240486234),
        dry_inertia: [
            SpatialInertia::from_raw(664),
            SpatialInertia::from_raw(264617),
            SpatialInertia::from_raw(264617),
        ],
        motor_aft_from_tail: SpatialPosition::from_raw(0),
        aft_rail_guide_from_tail: SpatialPosition::from_raw(2289),
        forward_rail_guide_from_tail: SpatialPosition::from_raw(6658),
        drogue_cda: SpatialArea::from_raw(132209742),
        main_cda: SpatialArea::from_raw(528838968),
        pitch_damping: SpatialCoefficient::from_raw(6660255),
        yaw_damping: SpatialCoefficient::from_raw(6660255),
        roll_damping: SpatialCoefficient::from_raw(1020927),
        source_manifest_identity: 3777114690,
        aero_knot_count: 3,
        aero_knots: knots,
    }
}
const fn motor() -> SpatialMotorPack {
    let mut knots = [SpatialMotorKnot::ZERO; KMP8_MAX_KNOTS];
    knots[0] = SpatialMotorKnot {
        time: SpatialTime::from_raw(0),
        thrust_raw_q13: 0,
    };
    knots[1] = SpatialMotorKnot {
        time: SpatialTime::from_raw(11534),
        thrust_raw_q13: 2108015,
    };
    knots[2] = SpatialMotorKnot {
        time: SpatialTime::from_raw(35127),
        thrust_raw_q13: 2421006,
    };
    knots[3] = SpatialMotorKnot {
        time: SpatialTime::from_raw(59245),
        thrust_raw_q13: 2425545,
    };
    knots[4] = SpatialMotorKnot {
        time: SpatialTime::from_raw(83362),
        thrust_raw_q13: 2442887,
    };
    knots[5] = SpatialMotorKnot {
        time: SpatialTime::from_raw(106955),
        thrust_raw_q13: 2417312,
    };
    knots[6] = SpatialMotorKnot {
        time: SpatialTime::from_raw(130810),
        thrust_raw_q13: 2356584,
    };
    knots[7] = SpatialMotorKnot {
        time: SpatialTime::from_raw(154927),
        thrust_raw_q13: 2314879,
    };
    knots[8] = SpatialMotorKnot {
        time: SpatialTime::from_raw(178782),
        thrust_raw_q13: 2235392,
    };
    knots[9] = SpatialMotorKnot {
        time: SpatialTime::from_raw(202637),
        thrust_raw_q13: 2187239,
    };
    knots[10] = SpatialMotorKnot {
        time: SpatialTime::from_raw(226492),
        thrust_raw_q13: 2110276,
    };
    knots[11] = SpatialMotorKnot {
        time: SpatialTime::from_raw(250348),
        thrust_raw_q13: 2052055,
    };
    knots[12] = SpatialMotorKnot {
        time: SpatialTime::from_raw(274465),
        thrust_raw_q13: 1954398,
    };
    knots[13] = SpatialMotorKnot {
        time: SpatialTime::from_raw(298320),
        thrust_raw_q13: 1872454,
    };
    knots[14] = SpatialMotorKnot {
        time: SpatialTime::from_raw(321913),
        thrust_raw_q13: 1762386,
    };
    knots[15] = SpatialMotorKnot {
        time: SpatialTime::from_raw(346030),
        thrust_raw_q13: 1622401,
    };
    knots[16] = SpatialMotorKnot {
        time: SpatialTime::from_raw(369885),
        thrust_raw_q13: 1479729,
    };
    knots[17] = SpatialMotorKnot {
        time: SpatialTime::from_raw(393740),
        thrust_raw_q13: 1321050,
    };
    knots[18] = SpatialMotorKnot {
        time: SpatialTime::from_raw(417595),
        thrust_raw_q13: 1201832,
    };
    knots[19] = SpatialMotorKnot {
        time: SpatialTime::from_raw(441450),
        thrust_raw_q13: 1101693,
    };
    knots[20] = SpatialMotorKnot {
        time: SpatialTime::from_raw(465568),
        thrust_raw_q13: 829366,
    };
    knots[21] = SpatialMotorKnot {
        time: SpatialTime::from_raw(489423),
        thrust_raw_q13: 431620,
    };
    knots[22] = SpatialMotorKnot {
        time: SpatialTime::from_raw(513016),
        thrust_raw_q13: 290497,
    };
    knots[23] = SpatialMotorKnot {
        time: SpatialTime::from_raw(537133),
        thrust_raw_q13: 199238,
    };
    knots[24] = SpatialMotorKnot {
        time: SpatialTime::from_raw(561250),
        thrust_raw_q13: 91464,
    };
    knots[25] = SpatialMotorKnot {
        time: SpatialTime::from_raw(585105),
        thrust_raw_q13: 37577,
    };
    knots[26] = SpatialMotorKnot {
        time: SpatialTime::from_raw(609223),
        thrust_raw_q13: 0,
    };
    SpatialMotorPack {
        identity: 2753027876,
        loaded_mass: SpatialMass::from_raw(978045),
        propellant_mass: SpatialMass::from_raw(518617),
        length: SpatialPosition::from_raw(2867),
        diameter: SpatialPosition::from_raw(442),
        loaded_cg_from_aft: SpatialMomentArm::from_raw(46976205),
        dry_cg_from_aft: SpatialMomentArm::from_raw(48318382),
        loaded_axial_inertia: SpatialInertia::from_raw(89),
        loaded_transverse_inertia: SpatialInertia::from_raw(2541),
        dry_axial_inertia: SpatialInertia::from_raw(42),
        dry_transverse_inertia: SpatialInertia::from_raw(1193),
        total_impulse_raw_q16: 28232909,
        burn_time: SpatialTime::from_raw(609223),
        knot_count: 27,
        knots,
    }
}
const fn wind() -> WindProfilePack {
    let mut knots = [WindKnot::ZERO; KWP8_MAX_WIND_KNOTS];
    knots[0] = WindKnot {
        altitude: SpatialPosition::from_raw(0),
        east: SpatialWind::from_raw(0),
        north: SpatialWind::from_raw(0),
    };
    knots[1] = WindKnot {
        altitude: SpatialPosition::from_raw(819200000),
        east: SpatialWind::from_raw(0),
        north: SpatialWind::from_raw(0),
    };
    WindProfilePack {
        identity: 1953842567,
        gust_seed: 0,
        gust_cadence: SpatialTime::from_raw(262144),
        gust_amplitude_east: SpatialWind::from_raw(0),
        gust_amplitude_north: SpatialWind::from_raw(0),
        max_gust: SpatialWind::from_raw(0),
        knot_count: 2,
        knots,
    }
}
pub static FIRESTORM_SPATIAL_VEHICLE: SpatialVehiclePack = vehicle();
pub static I211W_SPATIAL_MOTOR: SpatialMotorPack = motor();
pub static FIRESTORM_CALM_WIND: WindProfilePack = wind();
pub const FIRESTORM_I211_SPATIAL_MISSION: SpatialMissionPack = SpatialMissionPack {
    identity: 2349811339,
    vehicle_identity: 3569682864,
    motor_identity: 2753027876,
    wind_identity: 1953842567,
    environment_identity: 202565461,
    launch_altitude: SpatialPosition::from_raw(0),
    rail_length: SpatialPosition::from_raw(16384),
    launch_azimuth: SpatialAngle::from_raw(0),
    launch_elevation: SpatialAngle::from_raw(421657428),
    main_deployment_altitude: SpatialPosition::from_raw(1638400),
    drogue_inflation_time: SpatialTime::from_raw(131072),
    main_inflation_time: SpatialTime::from_raw(262144),
    max_mission_time: SpatialTime::from_raw(235929600),
    telemetry_period: SpatialTime::from_raw(65536),
    minimum_rail_exit_velocity: SpatialVelocity::from_raw(7864320),
    case_seed: 1263747384,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase8_format::{KMC8_LENGTH, KMP8_LENGTH, KVP8_LENGTH, KWP8_LENGTH};
    use crate::phase8_pack::{
        encode_spatial_mission_pack, encode_spatial_motor_pack, encode_spatial_vehicle_pack,
        encode_wind_profile_pack,
    };
    #[test]
    fn generated_constants_reencode_to_the_reviewed_packs() {
        let mut vehicle = [0u8; KVP8_LENGTH];
        let mut motor = [0u8; KMP8_LENGTH];
        let mut mission = [0u8; KMC8_LENGTH];
        let mut wind = [0u8; KWP8_LENGTH];
        encode_spatial_vehicle_pack(&FIRESTORM_SPATIAL_VEHICLE, &mut vehicle).unwrap();
        encode_spatial_motor_pack(&I211W_SPATIAL_MOTOR, &mut motor).unwrap();
        encode_spatial_mission_pack(FIRESTORM_I211_SPATIAL_MISSION, &mut mission).unwrap();
        encode_wind_profile_pack(&FIRESTORM_CALM_WIND, &mut wind).unwrap();
        assert_eq!(
            &vehicle,
            include_bytes!("../../phase8/examples/firestorm54.kvp8")
        );
        assert_eq!(
            &motor,
            include_bytes!("../../phase8/examples/aerotech-i211w.kmp8")
        );
        assert_eq!(
            &mission,
            include_bytes!("../../phase8/examples/firestorm-i211.kmc8")
        );
        assert_eq!(
            &wind,
            include_bytes!("../../phase8/examples/firestorm-calm.kwp8")
        );
    }
}
