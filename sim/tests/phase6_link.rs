use ksa64_sim::interface::phase6::{
    parse_link_frame, write_link_frame, EndpointRole, LinkFrame, LinkHeader, LinkRecordType,
    KLF6_MAX_DECODED, KLF6_MAX_ENCODED, KLF6_NONE,
};
use ksa64_sim::phase6_link::*;
#[test]
fn split_nominal_is_phase5_exact() {
    let e = run_exact_nominal().unwrap();
    assert_eq!(e.steps, 3133);
    assert_eq!(e.terminal_position_q12, [21468577, 3871182, 15698368]);
    assert_eq!(e.terminal_velocity_q24, [-66327286, 89767125, 68337641]);
    assert_eq!(e.sensor_checksum, 1741708362);
    assert_eq!(e.navigation_checksum, 2996014246);
    assert_eq!(e.flight_checksum, 4068248986)
}
fn frame(out: &mut [u8]) -> usize {
    let f = LinkFrame {
        header: LinkHeader {
            record_type: LinkRecordType::Heartbeat,
            flags: 0,
            session_id: 1,
            sequence: 2,
            acknowledgement: KLF6_NONE,
            measurement_epoch: 3,
            production_epoch: 4,
            effective_epoch: 5,
        },
        payload: &[1, 0, 2],
    };
    let mut d = [0u8; KLF6_MAX_DECODED];
    write_link_frame(&f, &mut d, out).unwrap()
}
#[test]
fn broker_faults_are_deterministic_and_detected() {
    let mut input = [0u8; KLF6_MAX_ENCODED];
    let n = frame(&mut input);
    let schedule = ImpairmentSchedule {
        corrupt_at: Some(0),
        drop_at: Some(1),
        duplicate_at: Some(2),
        disconnect_at: Some(3),
        delay_at: None,
    };
    let mut a = DeterministicBroker::new(schedule);
    let mut b = DeterministicBroker::new(schedule);
    let mut oa = [0u8; KLF6_MAX_ENCODED];
    let mut ob = [0u8; KLF6_MAX_ENCODED];
    for tick in 0..4 {
        let ra = a.route(
            EndpointRole::World,
            EndpointRole::Flight,
            &input[..n],
            &mut oa,
        );
        let rb = b.route(
            EndpointRole::World,
            EndpointRole::Flight,
            &input[..n],
            &mut ob,
        );
        assert_eq!(ra, rb);
        if let Ok((length, _)) = ra {
            assert_eq!(&oa[..length], &ob[..length]);
            if tick == 0 {
                let mut d = [0u8; KLF6_MAX_DECODED];
                assert!(parse_link_frame(&oa[..length], &mut d).is_err())
            }
        }
    }
    assert_eq!(a.transcript, b.transcript);
    assert_eq!(a.transcript.count, 4)
}
