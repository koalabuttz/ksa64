//! Passive Mission Control and independent delayed/noisy ground tracking.
use crate::phase6_link::ExactFrameObserver;
use ksa64_interface::phase6::{
    parse_link_frame, EndpointRole, GroundTrackingFix, LinkRecordType, KLF6_MAX_DECODED,
};

pub const GROUND_VALID_POSITION: u16 = 1;
pub const GROUND_VALID_VELOCITY: u16 = 2;
pub const GROUND_VALID_MASK: u16 = 3;
pub const MC_ALARM_PARSE: u16 = 1;
pub const MC_ALARM_SEQUENCE: u16 = 2;
const HASH_INITIAL: u32 = 2_166_136_261;

/// Observation-only endpoint. Its public API cannot produce flight commands.
pub struct PassiveMissionControl {
    frames: u32,
    world_frames: u32,
    flight_frames: u32,
    checksum: u32,
    alarms: u16,
    next_world: u32,
    next_flight: u32,
    decoded: [u8; KLF6_MAX_DECODED],
}
impl PassiveMissionControl {
    pub const fn new() -> Self {
        Self {
            frames: 0,
            world_frames: 0,
            flight_frames: 0,
            checksum: HASH_INITIAL,
            alarms: 0,
            next_world: 0,
            next_flight: 0,
            decoded: [0; KLF6_MAX_DECODED],
        }
    }
    pub const fn frames(&self) -> u32 {
        self.frames
    }
    pub const fn world_frames(&self) -> u32 {
        self.world_frames
    }
    pub const fn flight_frames(&self) -> u32 {
        self.flight_frames
    }
    pub const fn checksum(&self) -> u32 {
        self.checksum
    }
    pub const fn alarms(&self) -> u16 {
        self.alarms
    }
}
impl Default for PassiveMissionControl {
    fn default() -> Self {
        Self::new()
    }
}
impl ExactFrameObserver for PassiveMissionControl {
    fn observe(&mut self, source: EndpointRole, destination: EndpointRole, bytes: &[u8]) {
        let Ok(frame) = parse_link_frame(bytes, &mut self.decoded) else {
            self.alarms |= MC_ALARM_PARSE;
            return;
        };
        let (expected, record_ok) = match source {
            EndpointRole::World => (
                &mut self.next_world,
                frame.header.record_type == LinkRecordType::CanonicalSensor
                    && destination == EndpointRole::Flight,
            ),
            EndpointRole::Flight => (
                &mut self.next_flight,
                frame.header.record_type == LinkRecordType::CanonicalCommand
                    && destination == EndpointRole::World,
            ),
            _ => {
                self.alarms |= MC_ALARM_SEQUENCE;
                return;
            }
        };
        if !record_ok || frame.header.sequence != *expected {
            self.alarms |= MC_ALARM_SEQUENCE;
        }
        *expected = frame.header.sequence.wrapping_add(1);
        self.frames = self.frames.wrapping_add(1);
        if source == EndpointRole::World {
            self.world_frames += 1
        } else {
            self.flight_frames += 1
        }
        self.checksum = hash_bytes(hash_word(self.checksum, source as u32), bytes);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackingConfig {
    pub cadence_epochs: u16,
    pub delay_epochs: u16,
    pub network_id: u16,
}
impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            cadence_epochs: 8,
            delay_epochs: 3,
            network_id: 1,
        }
    }
}
/// Independent deterministic ground network noise and delivery delay.
pub struct GroundTrackingNetwork {
    config: TrackingConfig,
    rng: u32,
    next_id: u32,
    pending: Option<GroundTrackingFix>,
}
impl GroundTrackingNetwork {
    pub const fn new(seed: u32, config: TrackingConfig) -> Self {
        Self {
            config,
            rng: seed,
            next_id: 0,
            pending: None,
        }
    }
    pub fn observe(&mut self, epoch: u32, position_q12: [i32; 3], velocity_q24: [i32; 3]) {
        if self.config.cadence_epochs == 0
            || epoch % self.config.cadence_epochs as u32 != 0
            || self.pending.is_some()
        {
            return;
        }
        let mut p = position_q12;
        let mut v = velocity_q24;
        let mut axis = 0;
        while axis < 3 {
            p[axis] = p[axis].saturating_add(self.signed_sample().saturating_mul(6));
            v[axis] = v[axis].saturating_add(self.signed_sample().saturating_mul(256));
            axis += 1;
        }
        self.pending = Some(GroundTrackingFix {
            fix_id: self.next_id,
            measurement_epoch: epoch,
            production_epoch: epoch + self.config.delay_epochs as u32,
            position_ecef_q12: p,
            velocity_ecef_q24: v,
            network_id: self.config.network_id,
            validity: GROUND_VALID_MASK,
        });
        self.next_id = self.next_id.wrapping_add(1);
    }
    pub fn poll(&mut self, epoch: u32) -> Option<GroundTrackingFix> {
        if self
            .pending
            .map(|f| f.production_epoch <= epoch)
            .unwrap_or(false)
        {
            self.pending.take()
        } else {
            None
        }
    }
    fn signed_sample(&mut self) -> i32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 16) as u16 as i16 as i32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundEstimatorError {
    Invalid,
    Sequence,
    Future,
    Gap,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundEstimate {
    pub epoch: u32,
    pub position_q12: [i32; 3],
    pub velocity_q24: [i32; 3],
    pub fixes: u32,
    pub checksum: u32,
}
/// Ground-only alpha/beta estimator. It consumes tracking fixes, never simulator truth.
pub struct GroundEstimator {
    estimate: Option<GroundEstimate>,
    next_fix: u32,
}
impl GroundEstimator {
    pub const fn new() -> Self {
        Self {
            estimate: None,
            next_fix: 0,
        }
    }
    pub const fn estimate(&self) -> Option<GroundEstimate> {
        self.estimate
    }
    pub fn accept(
        &mut self,
        now: u32,
        fix: GroundTrackingFix,
    ) -> Result<GroundEstimate, GroundEstimatorError> {
        if fix.validity & GROUND_VALID_MASK != GROUND_VALID_MASK {
            return Err(GroundEstimatorError::Invalid);
        }
        if fix.fix_id != self.next_fix {
            return Err(GroundEstimatorError::Sequence);
        }
        if fix.measurement_epoch > fix.production_epoch || fix.production_epoch > now {
            return Err(GroundEstimatorError::Future);
        }
        let mut estimate = if let Some(mut e) = self.estimate {
            let gap = fix
                .measurement_epoch
                .checked_sub(e.epoch)
                .ok_or(GroundEstimatorError::Sequence)?;
            if gap > 4096 {
                return Err(GroundEstimatorError::Gap);
            }
            let mut tick = 0;
            while tick < gap {
                let mut a = 0;
                while a < 3 {
                    e.position_q12[a] = e.position_q12[a].saturating_add(e.velocity_q24[a] >> 15);
                    a += 1
                }
                tick += 1
            }
            let mut a = 0;
            while a < 3 {
                e.position_q12[a] = e.position_q12[a]
                    .saturating_add((fix.position_ecef_q12[a] - e.position_q12[a]) >> 1);
                e.velocity_q24[a] = e.velocity_q24[a]
                    .saturating_add((fix.velocity_ecef_q24[a] - e.velocity_q24[a]) >> 2);
                a += 1
            }
            e.epoch = fix.measurement_epoch;
            e.fixes += 1;
            e
        } else {
            GroundEstimate {
                epoch: fix.measurement_epoch,
                position_q12: fix.position_ecef_q12,
                velocity_q24: fix.velocity_ecef_q24,
                fixes: 1,
                checksum: HASH_INITIAL,
            }
        };
        estimate.checksum = hash_fix(estimate.checksum, fix);
        self.estimate = Some(estimate);
        self.next_fix = self.next_fix.wrapping_add(1);
        Ok(estimate)
    }
}
impl Default for GroundEstimator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundComparison {
    pub position_delta_q12: [i32; 3],
    pub velocity_delta_q24: [i32; 3],
}
pub fn compare_estimates(
    onboard_position_q12: [i32; 3],
    onboard_velocity_q24: [i32; 3],
    ground: GroundEstimate,
) -> GroundComparison {
    let mut p = [0; 3];
    let mut v = [0; 3];
    let mut a = 0;
    while a < 3 {
        p[a] = ground.position_q12[a].saturating_sub(onboard_position_q12[a]);
        v[a] = ground.velocity_q24[a].saturating_sub(onboard_velocity_q24[a]);
        a += 1
    }
    GroundComparison {
        position_delta_q12: p,
        velocity_delta_q24: v,
    }
}
fn hash_word(mut h: u32, v: u32) -> u32 {
    for b in v.to_le_bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16_777_619)
    }
    h
}
fn hash_bytes(mut h: u32, bytes: &[u8]) -> u32 {
    for b in bytes {
        h ^= *b as u32;
        h = h.wrapping_mul(16_777_619)
    }
    h
}
fn hash_fix(mut h: u32, fix: GroundTrackingFix) -> u32 {
    h = hash_word(h, fix.fix_id);
    h = hash_word(h, fix.measurement_epoch);
    h = hash_word(h, fix.production_epoch);
    for x in fix.position_ecef_q12 {
        h = hash_word(h, x as u32)
    }
    for x in fix.velocity_ecef_q24 {
        h = hash_word(h, x as u32)
    }
    h
}
