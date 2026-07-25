//! Exact Q18 event clock for the additive Phase 8.5 executor.

use crate::phase8_numeric::SpatialTime;

pub const AVIONICS_FAST_HZ: u8 = 32;
pub const AVIONICS_NAVIGATION_HZ: u8 = 8;
pub const AVIONICS_GUIDANCE_HZ: u8 = 1;
pub const AVIONICS_PERIOD_RAW_Q18: i32 = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactClockError {
    NegativeTime,
    NonIncreasingDeadline,
    ReleasePending,
    TimeMismatch,
    EpochOverflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactSegment {
    pub start: SpatialTime,
    pub end: SpatialTime,
    pub ends_at_release: bool,
    pub ends_at_physical_deadline: bool,
}

impl ExactSegment {
    pub const fn duration(self) -> SpatialTime {
        SpatialTime::from_raw(self.end.raw() - self.start.raw())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactEventClock {
    epoch: u32,
    next_release_raw: i32,
}

impl ExactEventClock {
    pub const fn new() -> Self {
        Self {
            epoch: 0,
            next_release_raw: 0,
        }
    }

    pub const fn epoch(self) -> u32 {
        self.epoch
    }
    pub const fn next_release(self) -> SpatialTime {
        SpatialTime::from_raw(self.next_release_raw)
    }
    pub const fn release_due(self, now: SpatialTime) -> bool {
        now.raw() == self.next_release_raw
    }

    pub fn consume_release(&mut self, now: SpatialTime) -> Result<u32, ExactClockError> {
        if now.raw() < 0 {
            return Err(ExactClockError::NegativeTime);
        }
        if now.raw() != self.next_release_raw {
            return Err(ExactClockError::TimeMismatch);
        }
        let released = self.epoch;
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or(ExactClockError::EpochOverflow)?;
        self.next_release_raw = i32::try_from(self.epoch)
            .ok()
            .and_then(|epoch| epoch.checked_mul(AVIONICS_PERIOD_RAW_Q18))
            .ok_or(ExactClockError::EpochOverflow)?;
        Ok(released)
    }

    pub fn next_segment(
        self,
        now: SpatialTime,
        physical_deadline: SpatialTime,
    ) -> Result<ExactSegment, ExactClockError> {
        if now.raw() < 0 {
            return Err(ExactClockError::NegativeTime);
        }
        if physical_deadline.raw() <= now.raw() {
            return Err(ExactClockError::NonIncreasingDeadline);
        }
        if self.release_due(now) {
            return Err(ExactClockError::ReleasePending);
        }
        let end_raw = physical_deadline.raw().min(self.next_release_raw);
        if end_raw <= now.raw() {
            return Err(ExactClockError::TimeMismatch);
        }
        Ok(ExactSegment {
            start: now,
            end: SpatialTime::from_raw(end_raw),
            ends_at_release: end_raw == self.next_release_raw,
            ends_at_physical_deadline: end_raw == physical_deadline.raw(),
        })
    }
}

impl Default for ExactEventClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_are_exact_q18_multiples() {
        let mut clock = ExactEventClock::new();
        for epoch in 0..512u32 {
            let now = SpatialTime::from_raw((epoch as i32) * AVIONICS_PERIOD_RAW_Q18);
            assert_eq!(clock.consume_release(now), Ok(epoch));
            assert_eq!(
                clock.next_release().raw(),
                ((epoch + 1) as i32) * AVIONICS_PERIOD_RAW_Q18
            );
        }
    }

    #[test]
    fn a_release_split_retains_the_original_physical_deadline() {
        let mut clock = ExactEventClock::new();
        assert_eq!(clock.consume_release(SpatialTime::ZERO), Ok(0));
        let mut now = SpatialTime::ZERO;
        for deadline in [2621, 5242, 7863] {
            let segment = clock
                .next_segment(now, SpatialTime::from_raw(deadline))
                .unwrap();
            assert!(segment.ends_at_physical_deadline);
            assert!(!segment.ends_at_release);
            now = segment.end;
        }
        let physical_deadline = SpatialTime::from_raw(10_484);
        let before = clock.next_segment(now, physical_deadline).unwrap();
        assert_eq!(before.duration().raw(), 329);
        assert!(before.ends_at_release);
        assert!(!before.ends_at_physical_deadline);
        now = before.end;
        assert_eq!(clock.consume_release(now), Ok(1));
        let after = clock.next_segment(now, physical_deadline).unwrap();
        assert_eq!(after.duration().raw(), 2_292);
        assert!(!after.ends_at_release);
        assert!(after.ends_at_physical_deadline);
    }

    #[test]
    fn pending_release_and_bad_deadlines_fail_closed() {
        let clock = ExactEventClock::new();
        assert_eq!(
            clock.next_segment(SpatialTime::ZERO, SpatialTime::from_raw(2621)),
            Err(ExactClockError::ReleasePending)
        );
        let mut consumed = clock;
        consumed.consume_release(SpatialTime::ZERO).unwrap();
        assert_eq!(
            consumed.next_segment(SpatialTime::ZERO, SpatialTime::ZERO),
            Err(ExactClockError::NonIncreasingDeadline)
        );
    }
}
