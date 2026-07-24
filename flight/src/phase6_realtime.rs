//! KSA-6R C64-oriented 32/8/1 Hz flight profile.
use ksa64_interface::phase6::{
    RealtimeAidCell, RealtimeCommandCell, RealtimeInertialCell, RealtimeStatusCell,
};
pub const PAL_CPU_HZ: u32 = 985_248;
pub const REALTIME_FAST_HZ: u32 = 32;
pub const REALTIME_NAV_HZ: u32 = 8;
pub const REALTIME_GUIDANCE_HZ: u32 = 1;
pub const PAL_FAST_TICK_CYCLES: u32 = PAL_CPU_HZ / REALTIME_FAST_HZ;
pub const PAL_TICK_BUDGET_CYCLES: u32 = PAL_FAST_TICK_CYCLES * 4 / 5;
pub const ALARM_STALE_LINK: u16 = 1;
pub const ALARM_DEADLINE: u16 = 2;
pub const ALARM_SAFEING: u16 = 4;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScheduleRelease {
    pub epoch: u16,
    pub fast: bool,
    pub navigation: bool,
    pub guidance: bool,
    pub status: bool,
}
pub struct VirtualScheduler {
    epoch: u16,
}
impl VirtualScheduler {
    pub const fn new() -> Self {
        Self { epoch: 0 }
    }
    pub fn release(&mut self) -> ScheduleRelease {
        let e = self.epoch;
        self.epoch = self.epoch.wrapping_add(1);
        ScheduleRelease {
            epoch: e,
            fast: true,
            navigation: e & 3 == 0,
            guidance: e & 31 == 0,
            status: e & 3 == 0,
        }
    }
}
impl Default for VirtualScheduler {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealtimeNavigation {
    pub position_q12: [i32; 3],
    pub velocity_q24: [i32; 3],
    pub platform_angle: [i16; 3],
    pub checksum: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealtimeFlightEvidence {
    pub command: RealtimeCommandCell,
    pub status: Option<RealtimeStatusCell>,
    pub release: ScheduleRelease,
    pub safe: bool,
}
pub struct RealtimeFlightComputer {
    session: u16,
    scheduler: VirtualScheduler,
    navigation: RealtimeNavigation,
    target: [i16; 3],
    delta_accumulator: [i32; 3],
    missing: u8,
    alarms: u16,
    deadline_misses: u16,
    flight_checksum: u32,
    safe: bool,
}
impl RealtimeFlightComputer {
    pub const fn new(session: u16, position: [i32; 3], velocity: [i32; 3]) -> Self {
        Self {
            session,
            scheduler: VirtualScheduler::new(),
            navigation: RealtimeNavigation {
                position_q12: position,
                velocity_q24: velocity,
                platform_angle: [0; 3],
                checksum: 2_166_136_261,
            },
            target: [0; 3],
            delta_accumulator: [0; 3],
            missing: 0,
            alarms: 0,
            deadline_misses: 0,
            flight_checksum: 2_166_136_261,
            safe: false,
        }
    }
    pub fn set_guidance_target(&mut self, target: [i16; 3]) {
        self.target = target
    }
    pub const fn navigation(&self) -> RealtimeNavigation {
        self.navigation
    }
    pub const fn is_safe(&self) -> bool {
        self.safe
    }
    pub const fn deadline_misses(&self) -> u16 {
        self.deadline_misses
    }
    pub fn record_cycles(&mut self, cycles: u32) {
        if cycles > PAL_TICK_BUDGET_CYCLES {
            self.deadline_misses = self.deadline_misses.saturating_add(1);
            self.alarms |= ALARM_DEADLINE | ALARM_SAFEING;
            self.safe = true
        }
    }
    pub fn tick(
        &mut self,
        inertial: Option<RealtimeInertialCell>,
        aid: Option<RealtimeAidCell>,
    ) -> RealtimeFlightEvidence {
        let release = self.scheduler.release();
        let epoch = release.epoch;
        if let Some(cell) =
            inertial.filter(|c| c.session == self.session && c.measurement_epoch == epoch)
        {
            self.missing = 0;
            self.navigation.platform_angle = cell.platform_angle;
            let mut a = 0;
            while a < 3 {
                self.delta_accumulator[a] =
                    self.delta_accumulator[a].saturating_add(cell.delta_velocity[a] as i32);
                a += 1
            }
        } else {
            self.missing = self.missing.saturating_add(1);
            self.alarms |= ALARM_STALE_LINK;
            if self.missing >= 3 {
                self.safe = true;
                self.alarms |= ALARM_SAFEING
            }
        }
        if release.navigation {
            if let Some(fix) = aid.filter(|c| c.session == self.session) {
                if fix.validity & 4 != 0 {
                    let mut a = 0;
                    while a < 3 {
                        self.navigation.position_q12[a] = self.navigation.position_q12[a]
                            .saturating_add(
                                (fix.gps_position_q12[a] - self.navigation.position_q12[a]) >> 2,
                            );
                        self.navigation.velocity_q24[a] = self.navigation.velocity_q24[a]
                            .saturating_add(
                                (fix.gps_velocity_q24[a] - self.navigation.velocity_q24[a]) >> 3,
                            );
                        a += 1
                    }
                }
            }
            let mut a = 0;
            while a < 3 {
                self.navigation.velocity_q24[a] =
                    self.navigation.velocity_q24[a].saturating_add(self.delta_accumulator[a] << 8);
                self.navigation.position_q12[a] = self.navigation.position_q12[a]
                    .saturating_add(self.navigation.velocity_q24[a] >> 15);
                self.delta_accumulator[a] = 0;
                a += 1
            }
            self.navigation.checksum = hash_nav(self.navigation)
        }
        let mut g = [0i16; 2];
        let mut r = [0i8; 3];
        if !self.safe && self.missing == 0 {
            let pitch = (self.target[1] as i32 - self.navigation.platform_angle[1] as i32) >> 2;
            let yaw = (self.target[2] as i32 - self.navigation.platform_angle[2] as i32) >> 2;
            g[0] = pitch.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            g[1] = yaw.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let roll = (self.target[0] as i32 - self.navigation.platform_angle[0] as i32) >> 7;
            r[0] = roll.clamp(-127, 127) as i8
        }
        let command = RealtimeCommandCell {
            session: self.session,
            source_epoch: epoch,
            effective_epoch: epoch.wrapping_add(1),
            flags: if self.safe { 1 } else { 0 },
            discrete: if self.safe { 4 } else { 0 },
            gimbal: g,
            rcs: r,
            status: self.missing,
        };
        self.flight_checksum = hash_command(self.flight_checksum, command);
        let status = if release.status {
            Some(RealtimeStatusCell {
                session: self.session,
                source_epoch: epoch,
                production_epoch: epoch,
                mode: if self.safe { 7 } else { 2 },
                flags: 0,
                alarms: self.alarms,
                navigation_position_q12: self.navigation.position_q12,
                navigation_velocity_q24: self.navigation.velocity_q24,
                flight_checksum: self.flight_checksum,
                deadline_misses: self.deadline_misses,
            })
        } else {
            None
        };
        RealtimeFlightEvidence {
            command,
            status,
            release,
            safe: self.safe,
        }
    }
}
fn hw(mut h: u32, v: u32) -> u32 {
    for b in v.to_le_bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16_777_619)
    }
    h
}
fn hash_nav(n: RealtimeNavigation) -> u32 {
    let mut h = n.checksum;
    for v in n.position_q12 {
        h = hw(h, v as u32)
    }
    for v in n.velocity_q24 {
        h = hw(h, v as u32)
    }
    for v in n.platform_angle {
        h = hw(h, v as u32)
    }
    h
}
fn hash_command(mut h: u32, c: RealtimeCommandCell) -> u32 {
    h = hw(h, c.source_epoch as u32);
    h = hw(h, c.effective_epoch as u32);
    h = hw(h, c.gimbal[0] as u32);
    h = hw(h, c.gimbal[1] as u32);
    h = hw(h, c.flags as u32);
    h
}
