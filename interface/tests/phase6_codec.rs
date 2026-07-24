use ksa64_interface::phase6::*;
fn h(t: LinkRecordType) -> LinkHeader {
    LinkHeader {
        record_type: t,
        flags: LINK_FLAG_ACK_REQUIRED,
        session_id: 0x12345678,
        sequence: 9,
        acknowledgement: 8,
        measurement_epoch: 100,
        production_epoch: 101,
        effective_epoch: 102,
    }
}
#[test]
fn klf6_round_trip_and_corruption() {
    let p = [0, 1, 2, 0, 4, 5, 255];
    let s = LinkFrame {
        header: h(LinkRecordType::CanonicalSensor),
        payload: &p,
    };
    let mut d = [0u8; KLF6_MAX_DECODED];
    let mut e = [0u8; KLF6_MAX_ENCODED];
    let n = write_link_frame(&s, &mut d, &mut e).unwrap();
    let mut r = [0u8; KLF6_MAX_DECODED];
    assert_eq!(parse_link_frame(&e[..n], &mut r).unwrap(), s);
    e[5] ^= 64;
    assert!(parse_link_frame(&e[..n], &mut r).is_err())
}
#[test]
fn maximum_frame_fits() {
    let p = [255u8; KLF6_MAX_PAYLOAD];
    let s = LinkFrame {
        header: h(LinkRecordType::CanonicalTelemetry),
        payload: &p,
    };
    let mut d = [0u8; KLF6_MAX_DECODED];
    let mut e = [0u8; KLF6_MAX_ENCODED];
    let n = write_link_frame(&s, &mut d, &mut e).unwrap();
    assert!(n <= KLF6_MAX_ENCODED);
    let mut r = [0u8; KLF6_MAX_DECODED];
    assert_eq!(parse_link_frame(&e[..n], &mut r).unwrap().payload, p)
}
#[test]
fn capability_and_cells_round_trip() {
    let c = LinkCapabilities {
        role: EndpointRole::Flight,
        mode: LinkMode::Realtime32,
        flags: CAP_REALTIME_32 | CAP_TRANSCRIPT,
        link_contract_id: PHASE6_LINK_CONTRACT_ID,
        vehicle_contract_id: 5,
        avionics_contract_id: PHASE6_REALTIME_CONTRACT_ID,
        max_payload: 512,
        fast_hz: 32,
        navigation_hz: 8,
        guidance_hz: 1,
    };
    let mut cb = [0u8; CAPABILITY_PAYLOAD_LENGTH];
    write_capabilities(&c, &mut cb).unwrap();
    assert_eq!(parse_capabilities(&cb), Ok(c));
    let i = RealtimeInertialCell {
        session: 7,
        measurement_epoch: 8,
        production_epoch: 9,
        validity: 3,
        flags: 1,
        platform_angle: [-1, 2, -3],
        angular_rate: [4, -5, 6],
        delta_velocity: [7, 8, -9],
        gimbal_applied: [10, -11],
        stage_status: 12,
    };
    let mut ib = [0u8; REALTIME_INERTIAL_LENGTH];
    write_realtime_inertial(&i, &mut ib).unwrap();
    assert_eq!(parse_realtime_inertial(&ib), Ok(i));
    let x = RealtimeCommandCell {
        session: 7,
        source_epoch: 8,
        effective_epoch: 9,
        flags: 1,
        discrete: 2,
        gimbal: [-100, 200],
        rcs: [-3, 4, 5],
        status: 6,
    };
    let mut xb = [0u8; REALTIME_COMMAND_LENGTH];
    write_realtime_command(&x, &mut xb).unwrap();
    assert_eq!(parse_realtime_command(&xb), Ok(x));
    let a = RealtimeAidCell {
        session: 7,
        measurement_epoch: 12,
        production_epoch: 13,
        validity: 4,
        events: 5,
        onboard_time_q16: 6,
        barometer_q12: 7,
        gps_position_q12: [8, 9, 10],
        gps_velocity_q24: [11, 12, 13],
        star_angle: [14, 15, 16],
        rcs_propellant_q12: 17,
        vehicle_status: 18,
    };
    let mut ab = [0u8; REALTIME_AID_LENGTH];
    write_realtime_aid(&a, &mut ab).unwrap();
    assert_eq!(parse_realtime_aid(&ab), Ok(a));
    let s = RealtimeStatusCell {
        session: 7,
        source_epoch: 20,
        production_epoch: 21,
        mode: 3,
        flags: 4,
        alarms: 5,
        navigation_position_q12: [6, 7, 8],
        navigation_velocity_q24: [9, 10, 11],
        flight_checksum: 12,
        deadline_misses: 13,
    };
    let mut sb = [0u8; REALTIME_STATUS_LENGTH];
    write_realtime_status(&s, &mut sb).unwrap();
    assert_eq!(parse_realtime_status(&sb), Ok(s))
}
