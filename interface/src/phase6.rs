//! Phase 6 transport, realtime-cell, and capability contracts.
use super::{crc32_ieee, CodecError};

pub const PHASE6_LINK_CONTRACT_ID: u32 = 0x0601_0001;
pub const PHASE6_REALTIME_CONTRACT_ID: u32 = 0x0602_0001;
pub const KLF6_HEADER_LENGTH: usize = 36;
pub const KLF6_MAX_PAYLOAD: usize = 512;
pub const KLF6_MAX_DECODED: usize = 552;
pub const KLF6_MAX_ENCODED: usize = 556;
pub const KLF6_NONE: u32 = u32::MAX;
pub const KLF6_MAGIC: [u8; 4] = *b"KLF6";
pub const LINK_FLAG_ACK_REQUIRED: u16 = 1;
pub const LINK_FLAG_RETRY: u16 = 2;
pub const LINK_FLAG_TERMINAL: u16 = 4;
pub const LINK_FLAG_DEGRADED: u16 = 8;
pub const LINK_FLAG_MASK: u16 = 15;
pub const CAP_EXACT_PACED: u32 = 1;
pub const CAP_REALTIME_32: u32 = 2;
pub const CAP_MISSION_CONTROL: u32 = 4;
pub const CAP_TRANSCRIPT: u32 = 8;
pub const CAP_REU_EVIDENCE: u32 = 16;
pub const CAP_MASK: u32 = 31;
pub const CAPABILITY_PAYLOAD_LENGTH: usize = 28;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EndpointRole {
    World = 1,
    Flight = 2,
    MissionControl = 3,
    Broker = 4,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LinkMode {
    ExactPaced = 1,
    Realtime32 = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LinkRecordType {
    Hello = 1,
    Capabilities = 2,
    Start = 3,
    CanonicalSensor = 4,
    CanonicalCommand = 5,
    Commit = 6,
    CanonicalTelemetry = 7,
    GroundTrackingFix = 8,
    Alarm = 9,
    Stop = 10,
    Heartbeat = 11,
}
impl LinkRecordType {
    fn parse(v: u8) -> Result<Self, CodecError> {
        match v {
            1 => Ok(Self::Hello),
            2 => Ok(Self::Capabilities),
            3 => Ok(Self::Start),
            4 => Ok(Self::CanonicalSensor),
            5 => Ok(Self::CanonicalCommand),
            6 => Ok(Self::Commit),
            7 => Ok(Self::CanonicalTelemetry),
            8 => Ok(Self::GroundTrackingFix),
            9 => Ok(Self::Alarm),
            10 => Ok(Self::Stop),
            11 => Ok(Self::Heartbeat),
            _ => Err(CodecError::Enum),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkHeader {
    pub record_type: LinkRecordType,
    pub flags: u16,
    pub session_id: u32,
    pub sequence: u32,
    pub acknowledgement: u32,
    pub measurement_epoch: u32,
    pub production_epoch: u32,
    pub effective_epoch: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkFrame<'a> {
    pub header: LinkHeader,
    pub payload: &'a [u8],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkCapabilities {
    pub role: EndpointRole,
    pub mode: LinkMode,
    pub flags: u32,
    pub link_contract_id: u32,
    pub vehicle_contract_id: u32,
    pub avionics_contract_id: u32,
    pub max_payload: u16,
    pub fast_hz: u8,
    pub navigation_hz: u8,
    pub guidance_hz: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkCodecError {
    Codec(CodecError),
    Cobs,
    Version,
    Magic,
    Capacity,
}
fn p16(o: &mut [u8], i: usize, v: u16) {
    o[i..i + 2].copy_from_slice(&v.to_le_bytes())
}
fn pi16(o: &mut [u8], i: usize, v: i16) {
    p16(o, i, v as u16)
}
fn p32(o: &mut [u8], i: usize, v: u32) {
    o[i..i + 4].copy_from_slice(&v.to_le_bytes())
}
fn pi32(o: &mut [u8], i: usize, v: i32) {
    p32(o, i, v as u32)
}
fn g16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn gi16(b: &[u8], i: usize) -> i16 {
    g16(b, i) as i16
}
fn g32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn gi32(b: &[u8], i: usize) -> i32 {
    g32(b, i) as i32
}

pub fn crc16_ccitt(bytes: &[u8]) -> u16 {
    let mut c = 0xffffu16;
    let mut i = 0;
    while i < bytes.len() {
        c ^= (bytes[i] as u16) << 8;
        let mut n = 0;
        while n < 8 {
            c = if c & 0x8000 != 0 {
                (c << 1) ^ 0x1021
            } else {
                c << 1
            };
            n += 1
        }
        i += 1
    }
    c
}
pub fn write_capabilities(v: &LinkCapabilities, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != CAPABILITY_PAYLOAD_LENGTH {
        return Err(CodecError::Length);
    }
    if v.flags & !CAP_MASK != 0
        || v.max_payload as usize > KLF6_MAX_PAYLOAD
        || v.fast_hz == 0
        || v.navigation_hz == 0
        || v.guidance_hz == 0
    {
        return Err(CodecError::Flags);
    }
    o.fill(0);
    o[0] = v.role as u8;
    o[1] = v.mode as u8;
    p32(o, 4, v.flags);
    p32(o, 8, v.link_contract_id);
    p32(o, 12, v.vehicle_contract_id);
    p32(o, 16, v.avionics_contract_id);
    p16(o, 20, v.max_payload);
    o[22] = v.fast_hz;
    o[23] = v.navigation_hz;
    o[24] = v.guidance_hz;
    Ok(())
}
pub fn parse_capabilities(b: &[u8]) -> Result<LinkCapabilities, CodecError> {
    if b.len() != CAPABILITY_PAYLOAD_LENGTH {
        return Err(CodecError::Length);
    }
    if b[2] != 0 || b[3] != 0 || b[25] != 0 || b[26] != 0 || b[27] != 0 {
        return Err(CodecError::Reserved);
    }
    let role = match b[0] {
        1 => EndpointRole::World,
        2 => EndpointRole::Flight,
        3 => EndpointRole::MissionControl,
        4 => EndpointRole::Broker,
        _ => return Err(CodecError::Enum),
    };
    let mode = match b[1] {
        1 => LinkMode::ExactPaced,
        2 => LinkMode::Realtime32,
        _ => return Err(CodecError::Enum),
    };
    let v = LinkCapabilities {
        role,
        mode,
        flags: g32(b, 4),
        link_contract_id: g32(b, 8),
        vehicle_contract_id: g32(b, 12),
        avionics_contract_id: g32(b, 16),
        max_payload: g16(b, 20),
        fast_hz: b[22],
        navigation_hz: b[23],
        guidance_hz: b[24],
    };
    if v.flags & !CAP_MASK != 0
        || v.max_payload as usize > KLF6_MAX_PAYLOAD
        || v.fast_hz == 0
        || v.navigation_hz == 0
        || v.guidance_hz == 0
    {
        return Err(CodecError::Flags);
    }
    Ok(v)
}

pub fn write_link_decoded(f: &LinkFrame<'_>, o: &mut [u8]) -> Result<usize, LinkCodecError> {
    if f.payload.len() > KLF6_MAX_PAYLOAD {
        return Err(LinkCodecError::Capacity);
    }
    if f.header.flags & !LINK_FLAG_MASK != 0 {
        return Err(LinkCodecError::Codec(CodecError::Flags));
    }
    let n = KLF6_HEADER_LENGTH + f.payload.len() + 4;
    if o.len() < n {
        return Err(LinkCodecError::Capacity);
    }
    o[..n].fill(0);
    o[..4].copy_from_slice(&KLF6_MAGIC);
    o[4] = 6;
    o[5] = f.header.record_type as u8;
    p16(o, 6, f.header.flags);
    p32(o, 8, f.header.session_id);
    p32(o, 12, f.header.sequence);
    p32(o, 16, f.header.acknowledgement);
    p32(o, 20, f.header.measurement_epoch);
    p32(o, 24, f.header.production_epoch);
    p32(o, 28, f.header.effective_epoch);
    p16(o, 32, f.payload.len() as u16);
    o[36..36 + f.payload.len()].copy_from_slice(f.payload);
    let c = 36 + f.payload.len();
    let crc = crc32_ieee(&o[..c]);
    p32(o, c, crc);
    Ok(n)
}
pub fn parse_link_decoded(b: &[u8]) -> Result<LinkFrame<'_>, LinkCodecError> {
    if b.len() < 40 || b.len() > KLF6_MAX_DECODED {
        return Err(LinkCodecError::Codec(CodecError::Length));
    }
    if b[..4] != KLF6_MAGIC {
        return Err(LinkCodecError::Magic);
    }
    if b[4] != 6 {
        return Err(LinkCodecError::Version);
    }
    if b[34] != 0 || b[35] != 0 {
        return Err(LinkCodecError::Codec(CodecError::Reserved));
    }
    let n = g16(b, 32) as usize;
    if n > KLF6_MAX_PAYLOAD || b.len() != 40 + n {
        return Err(LinkCodecError::Codec(CodecError::Length));
    }
    let c = 36 + n;
    if crc32_ieee(&b[..c]) != g32(b, c) {
        return Err(LinkCodecError::Codec(CodecError::Checksum));
    }
    let flags = g16(b, 6);
    if flags & !LINK_FLAG_MASK != 0 {
        return Err(LinkCodecError::Codec(CodecError::Flags));
    }
    Ok(LinkFrame {
        header: LinkHeader {
            record_type: LinkRecordType::parse(b[5]).map_err(LinkCodecError::Codec)?,
            flags,
            session_id: g32(b, 8),
            sequence: g32(b, 12),
            acknowledgement: g32(b, 16),
            measurement_epoch: g32(b, 20),
            production_epoch: g32(b, 24),
            effective_epoch: g32(b, 28),
        },
        payload: &b[36..c],
    })
}
pub fn cobs_encode(input: &[u8], out: &mut [u8]) -> Result<usize, LinkCodecError> {
    if out.is_empty() {
        return Err(LinkCodecError::Capacity);
    }
    let (mut r, mut w, mut at, mut code) = (0usize, 1usize, 0usize, 1u8);
    while r < input.len() {
        if input[r] == 0 {
            if at >= out.len() {
                return Err(LinkCodecError::Capacity);
            }
            out[at] = code;
            at = w;
            w += 1;
            code = 1
        } else {
            if w >= out.len() {
                return Err(LinkCodecError::Capacity);
            }
            out[w] = input[r];
            w += 1;
            code = code.wrapping_add(1);
            if code == 0xff {
                out[at] = code;
                at = w;
                w += 1;
                code = 1
            }
        }
        r += 1
    }
    if at >= out.len() || w >= out.len() {
        return Err(LinkCodecError::Capacity);
    }
    out[at] = code;
    out[w] = 0;
    Ok(w + 1)
}
pub fn cobs_decode(input: &[u8], out: &mut [u8]) -> Result<usize, LinkCodecError> {
    if input.is_empty() || input[input.len() - 1] != 0 {
        return Err(LinkCodecError::Cobs);
    }
    let (mut r, mut w) = (0usize, 0usize);
    let end = input.len() - 1;
    while r < end {
        let code = input[r] as usize;
        if code == 0 {
            return Err(LinkCodecError::Cobs);
        }
        r += 1;
        let n = code - 1;
        if r + n > end || w + n > out.len() {
            return Err(LinkCodecError::Cobs);
        }
        out[w..w + n].copy_from_slice(&input[r..r + n]);
        r += n;
        w += n;
        if code != 0xff && r < end {
            if w >= out.len() {
                return Err(LinkCodecError::Capacity);
            }
            out[w] = 0;
            w += 1
        }
    }
    Ok(w)
}
pub fn write_link_frame(
    f: &LinkFrame<'_>,
    decoded: &mut [u8],
    encoded: &mut [u8],
) -> Result<usize, LinkCodecError> {
    let n = write_link_decoded(f, decoded)?;
    cobs_encode(&decoded[..n], encoded)
}
pub fn parse_link_frame<'a>(
    encoded: &[u8],
    decoded: &'a mut [u8],
) -> Result<LinkFrame<'a>, LinkCodecError> {
    let n = cobs_decode(encoded, decoded)?;
    parse_link_decoded(&decoded[..n])
}

pub const REALTIME_INERTIAL_LENGTH: usize = 40;
pub const REALTIME_COMMAND_LENGTH: usize = 24;
pub const REALTIME_AID_LENGTH: usize = 64;
pub const REALTIME_STATUS_LENGTH: usize = 48;
pub const KLR6_SYNC: [u8; 2] = [0xd6, 0x5a];
fn cell_prefix(o: &mut [u8], kind: u8, session: u16, a: u16, b: u16) {
    o[..2].copy_from_slice(&KLR6_SYNC);
    o[2] = 6;
    o[3] = kind;
    p16(o, 4, session);
    p16(o, 6, a);
    p16(o, 8, b)
}
fn cell_check(b: &[u8], n: usize, kind: u8) -> Result<(), CodecError> {
    if b.len() != n {
        return Err(CodecError::Length);
    }
    if b[..2] != KLR6_SYNC || b[2] != 6 || b[3] != kind {
        return Err(CodecError::Enum);
    }
    if crc16_ccitt(&b[..n - 2]) != g16(b, n - 2) {
        return Err(CodecError::Checksum);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealtimeInertialCell {
    pub session: u16,
    pub measurement_epoch: u16,
    pub production_epoch: u16,
    pub validity: u8,
    pub flags: u8,
    pub platform_angle: [i16; 3],
    pub angular_rate: [i16; 3],
    pub delta_velocity: [i16; 3],
    pub gimbal_applied: [i16; 2],
    pub stage_status: u16,
}
pub fn write_realtime_inertial(v: &RealtimeInertialCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != 40 {
        return Err(CodecError::Length);
    }
    o.fill(0);
    cell_prefix(o, 1, v.session, v.measurement_epoch, v.production_epoch);
    o[10] = v.validity;
    o[11] = v.flags;
    let mut a = 0;
    while a < 3 {
        pi16(o, 12 + a * 2, v.platform_angle[a]);
        pi16(o, 18 + a * 2, v.angular_rate[a]);
        pi16(o, 24 + a * 2, v.delta_velocity[a]);
        a += 1
    }
    pi16(o, 30, v.gimbal_applied[0]);
    pi16(o, 32, v.gimbal_applied[1]);
    p16(o, 34, v.stage_status);
    let c = crc16_ccitt(&o[..38]);
    p16(o, 38, c);
    Ok(())
}
pub fn parse_realtime_inertial(b: &[u8]) -> Result<RealtimeInertialCell, CodecError> {
    cell_check(b, 40, 1)?;
    if b[36] != 0 || b[37] != 0 {
        return Err(CodecError::Reserved);
    }
    let (mut p, mut r, mut d) = ([0; 3], [0; 3], [0; 3]);
    let mut a = 0;
    while a < 3 {
        p[a] = gi16(b, 12 + a * 2);
        r[a] = gi16(b, 18 + a * 2);
        d[a] = gi16(b, 24 + a * 2);
        a += 1
    }
    Ok(RealtimeInertialCell {
        session: g16(b, 4),
        measurement_epoch: g16(b, 6),
        production_epoch: g16(b, 8),
        validity: b[10],
        flags: b[11],
        platform_angle: p,
        angular_rate: r,
        delta_velocity: d,
        gimbal_applied: [gi16(b, 30), gi16(b, 32)],
        stage_status: g16(b, 34),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealtimeCommandCell {
    pub session: u16,
    pub source_epoch: u16,
    pub effective_epoch: u16,
    pub flags: u8,
    pub discrete: u8,
    pub gimbal: [i16; 2],
    pub rcs: [i8; 3],
    pub status: u8,
}
pub fn write_realtime_command(v: &RealtimeCommandCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != 24 {
        return Err(CodecError::Length);
    }
    o.fill(0);
    cell_prefix(o, 2, v.session, v.source_epoch, v.effective_epoch);
    o[10] = v.flags;
    o[11] = v.discrete;
    pi16(o, 12, v.gimbal[0]);
    pi16(o, 14, v.gimbal[1]);
    o[16] = v.rcs[0] as u8;
    o[17] = v.rcs[1] as u8;
    o[18] = v.rcs[2] as u8;
    o[19] = v.status;
    let c = crc16_ccitt(&o[..22]);
    p16(o, 22, c);
    Ok(())
}
pub fn parse_realtime_command(b: &[u8]) -> Result<RealtimeCommandCell, CodecError> {
    cell_check(b, 24, 2)?;
    if b[20] != 0 || b[21] != 0 {
        return Err(CodecError::Reserved);
    }
    Ok(RealtimeCommandCell {
        session: g16(b, 4),
        source_epoch: g16(b, 6),
        effective_epoch: g16(b, 8),
        flags: b[10],
        discrete: b[11],
        gimbal: [gi16(b, 12), gi16(b, 14)],
        rcs: [b[16] as i8, b[17] as i8, b[18] as i8],
        status: b[19],
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealtimeAidCell {
    pub session: u16,
    pub measurement_epoch: u16,
    pub production_epoch: u16,
    pub validity: u16,
    pub events: u16,
    pub onboard_time_q16: i32,
    pub barometer_q12: i32,
    pub gps_position_q12: [i32; 3],
    pub gps_velocity_q24: [i32; 3],
    pub star_angle: [i16; 3],
    pub rcs_propellant_q12: i32,
    pub vehicle_status: u32,
}
pub fn write_realtime_aid(v: &RealtimeAidCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != 64 {
        return Err(CodecError::Length);
    }
    o.fill(0);
    cell_prefix(o, 3, v.session, v.measurement_epoch, v.production_epoch);
    p16(o, 10, v.validity);
    p16(o, 12, v.events);
    pi32(o, 14, v.onboard_time_q16);
    pi32(o, 18, v.barometer_q12);
    let mut a = 0;
    while a < 3 {
        pi32(o, 22 + a * 4, v.gps_position_q12[a]);
        pi32(o, 34 + a * 4, v.gps_velocity_q24[a]);
        pi16(o, 46 + a * 2, v.star_angle[a]);
        a += 1
    }
    pi32(o, 52, v.rcs_propellant_q12);
    p32(o, 56, v.vehicle_status);
    let c = crc16_ccitt(&o[..62]);
    p16(o, 62, c);
    Ok(())
}
pub fn parse_realtime_aid(b: &[u8]) -> Result<RealtimeAidCell, CodecError> {
    cell_check(b, 64, 3)?;
    if b[60] != 0 || b[61] != 0 {
        return Err(CodecError::Reserved);
    }
    let (mut p, mut v, mut s) = ([0; 3], [0; 3], [0; 3]);
    let mut a = 0;
    while a < 3 {
        p[a] = gi32(b, 22 + a * 4);
        v[a] = gi32(b, 34 + a * 4);
        s[a] = gi16(b, 46 + a * 2);
        a += 1
    }
    Ok(RealtimeAidCell {
        session: g16(b, 4),
        measurement_epoch: g16(b, 6),
        production_epoch: g16(b, 8),
        validity: g16(b, 10),
        events: g16(b, 12),
        onboard_time_q16: gi32(b, 14),
        barometer_q12: gi32(b, 18),
        gps_position_q12: p,
        gps_velocity_q24: v,
        star_angle: s,
        rcs_propellant_q12: gi32(b, 52),
        vehicle_status: g32(b, 56),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealtimeStatusCell {
    pub session: u16,
    pub source_epoch: u16,
    pub production_epoch: u16,
    pub mode: u8,
    pub flags: u8,
    pub alarms: u16,
    pub navigation_position_q12: [i32; 3],
    pub navigation_velocity_q24: [i32; 3],
    pub flight_checksum: u32,
    pub deadline_misses: u16,
}
pub fn write_realtime_status(v: &RealtimeStatusCell, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != 48 {
        return Err(CodecError::Length);
    }
    o.fill(0);
    cell_prefix(o, 4, v.session, v.source_epoch, v.production_epoch);
    o[10] = v.mode;
    o[11] = v.flags;
    p16(o, 12, v.alarms);
    let mut a = 0;
    while a < 3 {
        pi32(o, 14 + a * 4, v.navigation_position_q12[a]);
        pi32(o, 26 + a * 4, v.navigation_velocity_q24[a]);
        a += 1
    }
    p32(o, 38, v.flight_checksum);
    p16(o, 42, v.deadline_misses);
    let c = crc16_ccitt(&o[..46]);
    p16(o, 46, c);
    Ok(())
}
pub fn parse_realtime_status(b: &[u8]) -> Result<RealtimeStatusCell, CodecError> {
    cell_check(b, 48, 4)?;
    if b[44] != 0 || b[45] != 0 {
        return Err(CodecError::Reserved);
    }
    let (mut p, mut v) = ([0; 3], [0; 3]);
    let mut a = 0;
    while a < 3 {
        p[a] = gi32(b, 14 + a * 4);
        v[a] = gi32(b, 26 + a * 4);
        a += 1
    }
    Ok(RealtimeStatusCell {
        session: g16(b, 4),
        source_epoch: g16(b, 6),
        production_epoch: g16(b, 8),
        mode: b[10],
        flags: b[11],
        alarms: g16(b, 12),
        navigation_position_q12: p,
        navigation_velocity_q24: v,
        flight_checksum: g32(b, 38),
        deadline_misses: g16(b, 42),
    })
}

pub const GROUND_TRACKING_FIX_LENGTH: usize = 40;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundTrackingFix {
    pub fix_id: u32,
    pub measurement_epoch: u32,
    pub production_epoch: u32,
    pub position_ecef_q12: [i32; 3],
    pub velocity_ecef_q24: [i32; 3],
    pub network_id: u16,
    pub validity: u16,
}
pub fn write_ground_tracking_fix(v: &GroundTrackingFix, o: &mut [u8]) -> Result<(), CodecError> {
    if o.len() != 40 {
        return Err(CodecError::Length);
    }
    o.fill(0);
    p32(o, 0, v.fix_id);
    p32(o, 4, v.measurement_epoch);
    p32(o, 8, v.production_epoch);
    let mut a = 0;
    while a < 3 {
        pi32(o, 12 + a * 4, v.position_ecef_q12[a]);
        pi32(o, 24 + a * 4, v.velocity_ecef_q24[a]);
        a += 1
    }
    p16(o, 36, v.network_id);
    p16(o, 38, v.validity);
    Ok(())
}
pub fn parse_ground_tracking_fix(b: &[u8]) -> Result<GroundTrackingFix, CodecError> {
    if b.len() != 40 {
        return Err(CodecError::Length);
    }
    let (mut p, mut v) = ([0; 3], [0; 3]);
    let mut a = 0;
    while a < 3 {
        p[a] = gi32(b, 12 + a * 4);
        v[a] = gi32(b, 24 + a * 4);
        a += 1
    }
    Ok(GroundTrackingFix {
        fix_id: g32(b, 0),
        measurement_epoch: g32(b, 4),
        production_epoch: g32(b, 8),
        position_ecef_q12: p,
        velocity_ecef_q24: v,
        network_id: g16(b, 36),
        validity: g16(b, 38),
    })
}
