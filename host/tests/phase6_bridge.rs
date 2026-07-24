use ksa64_flight::phase6_realtime::{reference_realtime_guidance_slice, RealtimeFlightComputer};
use ksa64_host::phase6::{configure_socket, run_realtime_world_bridge};
use ksa64_interface::phase6::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn flight_peer(mut stream: TcpStream) -> (u32, u32, [i32; 3], [i32; 3]) {
    configure_socket(&stream).unwrap();
    stream.write_all(&KLR6_READY).unwrap();
    stream.flush().unwrap();
    let mut flight =
        RealtimeFlightComputer::new(0x6a52, [22_958_965, 0, 12_465_701], [0, 6_857_499, 0]);
    let s = reference_realtime_guidance_slice(0);
    flight.set_guidance_segment(s.start, s.end, s.rate);
    for epoch in 0u16.. {
        let aid = if epoch & 3 == 0 {
            let mut b = [0u8; REALTIME_AID_LENGTH];
            stream.read_exact(&mut b).unwrap();
            Some(parse_realtime_aid(&b).unwrap())
        } else {
            None
        };
        let mut b = [0u8; REALTIME_INERTIAL_LENGTH];
        stream.read_exact(&mut b).unwrap();
        let inertial = parse_realtime_inertial(&b).unwrap();
        if epoch & 31 == 2 {
            let s = reference_realtime_guidance_slice(epoch >> 5);
            flight.set_guidance_segment(s.start, s.end, s.rate);
        }
        let out = flight.tick(Some(inertial), aid);
        let mut c = [0u8; REALTIME_COMMAND_LENGTH];
        write_realtime_command(&out.command, &mut c).unwrap();
        stream.write_all(&c).unwrap();
        if let Some(status) = out.status {
            let mut s = [0u8; REALTIME_STATUS_LENGTH];
            write_realtime_status(&status, &mut s).unwrap();
            stream.write_all(&s).unwrap();
        }
        stream.flush().unwrap();
        if inertial.flags & 1 != 0 {
            let n = flight.navigation();
            return (
                flight.flight_checksum(),
                n.checksum,
                n.position_q12,
                n.velocity_q24,
            );
        }
    }
    unreachable!()
}
#[test]
fn full_tcp_bridge_matches_frozen_realtime_mission() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let flight = thread::spawn(move || flight_peer(TcpStream::connect(addr).unwrap()));
    let (mut socket, _) = listener.accept().unwrap();
    configure_socket(&socket).unwrap();
    let e = run_realtime_world_bridge(&mut socket, u32::MAX).unwrap();
    let f = flight.join().unwrap();
    assert_eq!(e.fast_epochs, 12692);
    assert_eq!(e.mission_steps, 3173);
    assert_eq!(e.terminal_position_q12, [21360371, 4030786, 15731027]);
    assert_eq!(e.terminal_velocity_q24, [-69442203, 96406364, 65655653]);
    assert_eq!(f.0, 2901449607);
    assert_eq!(f.1, 2195755368);
    assert_eq!(f.2, [21360000, 4031445, 15731484]);
    assert_eq!(f.3, [-68076267, 95786604, 65320561]);
    assert_eq!(e.final_flight_checksum, 2901449607);
    assert_eq!(e.navigation_checksum, 2195755368);
    assert_eq!(e.deadline_misses, 0);
    assert_eq!(e.alarms, 0);
}
