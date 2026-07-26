//! Sampled propulsion and changing mass properties for Phase 8.

use crate::numeric::{add, divide_scaled, multiply_scaled, subtract, NumericStatus};
use crate::phase8_numeric::{SpatialInertia, SpatialMass, SpatialMomentArm, SpatialTime};
use crate::phase8_pack::{SpatialMotorPack, SpatialVehiclePack};
use crate::phase9_5_contract::AdvancedEffectorPack;

use super::SpatialMassProperties;

pub(super) fn scale_ppm(value: i32, ppm: i32, status: &mut NumericStatus) -> i32 {
    let scale_q20 = divide_scaled(ppm, 1_000_000, 20, status);
    multiply_scaled(value, scale_q20, 20, status)
}

pub(super) fn sample_motor_thrust(
    motor: &SpatialMotorPack,
    time: SpatialTime,
    scale: i32,
    status: &mut NumericStatus,
) -> i32 {
    if time.raw() >= motor.burn_time.raw() {
        return 0;
    }
    let count = motor.knot_count as usize;
    let mut index = 0usize;
    while index + 1 < count && time.raw() > motor.knots[index + 1].time.raw() {
        index += 1;
    }
    let low = motor.knots[index];
    let high = motor.knots[(index + 1).min(count - 1)];
    let thrust = if low.time == high.time {
        low.thrust_raw_q13
    } else {
        let fraction = divide_scaled(
            subtract(time.raw(), low.time.raw(), status),
            subtract(high.time.raw(), low.time.raw(), status),
            16,
            status,
        )
        .clamp(0, 65_536);
        add(
            low.thrust_raw_q13,
            multiply_scaled(
                subtract(high.thrust_raw_q13, low.thrust_raw_q13, status),
                fraction,
                16,
                status,
            ),
            status,
        )
    };
    scale_ppm(thrust, scale, status)
}

fn interpolate_raw(
    loaded: i32,
    dry: i32,
    prop_fraction_q21: i32,
    status: &mut NumericStatus,
) -> i32 {
    add(
        dry,
        multiply_scaled(subtract(loaded, dry, status), prop_fraction_q21, 21, status),
        status,
    )
}

pub(super) fn derive_mass_properties(
    vehicle: &SpatialVehiclePack,
    motor: &SpatialMotorPack,
    propellant_remaining: SpatialMass,
    mass_scale_ppm: i32,
    status: &mut NumericStatus,
) -> SpatialMassProperties {
    let motor_dry_mass = subtract(motor.loaded_mass.raw(), motor.propellant_mass.raw(), status);
    let motor_mass = add(motor_dry_mass, propellant_remaining.raw(), status);
    let fraction_q21 = divide_scaled(
        propellant_remaining.raw(),
        motor.propellant_mass.raw(),
        21,
        status,
    )
    .clamp(0, 1 << 21);
    let motor_cg_aft_q28 = interpolate_raw(
        motor.loaded_cg_from_aft.raw(),
        motor.dry_cg_from_aft.raw(),
        fraction_q21,
        status,
    );
    let motor_cg_nose_q28 = subtract(
        (vehicle.length.raw() << 15) - (vehicle.motor_aft_from_tail.raw() << 15),
        motor_cg_aft_q28,
        status,
    );
    let total_unscaled = add(vehicle.dry_mass.raw(), motor_mass, status);
    let first_vehicle = multiply_scaled(
        vehicle.dry_mass.raw(),
        vehicle.dry_cg_from_nose.raw(),
        21,
        status,
    );
    let first_motor = multiply_scaled(motor_mass, motor_cg_nose_q28, 21, status);
    let cg_q28 = divide_scaled(
        add(first_vehicle, first_motor, status),
        total_unscaled,
        21,
        status,
    );
    let motor_inertia = [
        interpolate_raw(
            motor.loaded_axial_inertia.raw(),
            motor.dry_axial_inertia.raw(),
            fraction_q21,
            status,
        ),
        interpolate_raw(
            motor.loaded_transverse_inertia.raw(),
            motor.dry_transverse_inertia.raw(),
            fraction_q21,
            status,
        ),
    ];
    let vehicle_offset = subtract(vehicle.dry_cg_from_nose.raw(), cg_q28, status);
    let motor_offset = subtract(motor_cg_nose_q28, cg_q28, status);
    let vehicle_d2_q28 = multiply_scaled(vehicle_offset, vehicle_offset, 28, status);
    let motor_d2_q28 = multiply_scaled(motor_offset, motor_offset, 28, status);
    let vehicle_parallel_q19 = multiply_scaled(vehicle.dry_mass.raw(), vehicle_d2_q28, 30, status);
    let motor_parallel_q19 = multiply_scaled(motor_mass, motor_d2_q28, 30, status);
    let transverse = add(
        add(vehicle.dry_inertia[1].raw(), motor_inertia[1], status),
        add(vehicle_parallel_q19, motor_parallel_q19, status),
        status,
    );
    SpatialMassProperties {
        mass: SpatialMass::from_raw(scale_ppm(total_unscaled, mass_scale_ppm, status)),
        cg_from_nose: SpatialMomentArm::from_raw(cg_q28),
        inertia: [
            SpatialInertia::from_raw(add(vehicle.dry_inertia[0].raw(), motor_inertia[0], status)),
            SpatialInertia::from_raw(transverse),
            SpatialInertia::from_raw(transverse),
        ],
        propellant_remaining,
    }
}

pub(super) fn add_rcs_propellant_mass(
    base: SpatialMassProperties,
    pack: &AdvancedEffectorPack,
    remaining_q21: i32,
    status: &mut NumericStatus,
) -> SpatialMassProperties {
    if !pack.set.has_rcs() || remaining_q21 <= 0 {
        return base;
    }
    let remaining = remaining_q21.clamp(0, pack.propellant_wet_mass_q21);
    let total = add(base.mass.raw(), remaining, status);
    let base_first = multiply_scaled(base.mass.raw(), base.cg_from_nose.raw(), 21, status);
    let tank_first = multiply_scaled(remaining, pack.tank_position_q28[0], 21, status);
    let cg = divide_scaled(add(base_first, tank_first, status), total, 21, status);
    let base_dx = subtract(base.cg_from_nose.raw(), cg, status);
    let tank_dx = subtract(pack.tank_position_q28[0], cg, status);
    let base_d2 = multiply_scaled(base_dx, base_dx, 28, status);
    let tank_x2 = multiply_scaled(tank_dx, tank_dx, 28, status);
    let tank_y2 = multiply_scaled(
        pack.tank_position_q28[1],
        pack.tank_position_q28[1],
        28,
        status,
    );
    let tank_z2 = multiply_scaled(
        pack.tank_position_q28[2],
        pack.tank_position_q28[2],
        28,
        status,
    );
    let base_parallel = multiply_scaled(base.mass.raw(), base_d2, 30, status);
    let tank_axial = multiply_scaled(remaining, add(tank_y2, tank_z2, status), 30, status);
    let tank_pitch = multiply_scaled(remaining, add(tank_x2, tank_z2, status), 30, status);
    let tank_yaw = multiply_scaled(remaining, add(tank_x2, tank_y2, status), 30, status);
    SpatialMassProperties {
        mass: SpatialMass::from_raw(total),
        cg_from_nose: SpatialMomentArm::from_raw(cg),
        inertia: [
            SpatialInertia::from_raw(add(base.inertia[0].raw(), tank_axial, status)),
            SpatialInertia::from_raw(add(
                add(base.inertia[1].raw(), base_parallel, status),
                tank_pitch,
                status,
            )),
            SpatialInertia::from_raw(add(
                add(base.inertia[2].raw(), base_parallel, status),
                tank_yaw,
                status,
            )),
        ],
        propellant_remaining: base.propellant_remaining,
    }
}

pub(super) fn burn_propellant(
    motor: &SpatialMotorPack,
    remaining: SpatialMass,
    thrust_q13: i32,
    timestep: SpatialTime,
    status: &mut NumericStatus,
) -> SpatialMass {
    if thrust_q13 <= 0 || remaining.raw() <= 0 {
        return remaining;
    }
    let impulse_q16 = multiply_scaled(thrust_q13, timestep.raw(), 15, status);
    let burn_fraction_q21 = divide_scaled(impulse_q16, motor.total_impulse_raw_q16, 21, status);
    let burned = multiply_scaled(motor.propellant_mass.raw(), burn_fraction_q21, 21, status);
    SpatialMass::from_raw((remaining.raw() - burned).max(0))
}
