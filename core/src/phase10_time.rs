//! Leap-aware UTC/TAI/TT/UT1 contracts for Phase 10.
//!
//! Calendar parsing and EOP ingestion are host-compiler responsibilities. The
//! portable path consumes bounded day/second records and integrates elapsed
//! TAI only.

use crate::phase10_numeric::MissionTimeQ16;

pub const MAX_LEAP_TRANSITIONS: usize = 32;
pub const SECONDS_PER_DAY: i64 = 86_400;
pub const TT_MINUS_TAI_Q16: i64 = 2_109_358; // 32.184 seconds

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum TimeScaleId {
    #[default]
    Utc = 1,
    Tai = 2,
    Tt = 3,
    Ut1 = 4,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct UtcInstant {
    /// Days since 1970-01-01 in the proleptic Gregorian calendar.
    pub unix_day: i32,
    /// 0..=86400; 86400 is accepted only on a declared positive leap day.
    pub second_of_day: u32,
    pub subsecond_q16: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct LeapSecondTransition {
    /// UTC day on which the new offset is in force at 00:00:00.
    pub effective_unix_day: i32,
    pub tai_minus_utc_after: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct LeapSecondTable {
    pub count: u8,
    pub initial_tai_minus_utc: i16,
    pub transitions: [LeapSecondTransition; MAX_LEAP_TRANSITIONS],
}

impl LeapSecondTable {
    pub const EMPTY: Self = Self {
        count: 0,
        initial_tai_minus_utc: 10,
        transitions: [LeapSecondTransition {
            effective_unix_day: 0,
            tai_minus_utc_after: 0,
        }; MAX_LEAP_TRANSITIONS],
    };

    pub fn validate(&self) -> Result<(), TimeError> {
        if self.count as usize > MAX_LEAP_TRANSITIONS {
            return Err(TimeError::Table);
        }
        let mut previous_day = i32::MIN;
        let mut previous_offset = self.initial_tai_minus_utc;
        for transition in &self.transitions[..self.count as usize] {
            if transition.effective_unix_day <= previous_day
                || transition.tai_minus_utc_after != previous_offset + 1
            {
                return Err(TimeError::Table);
            }
            previous_day = transition.effective_unix_day;
            previous_offset = transition.tai_minus_utc_after;
        }
        if self.transitions[self.count as usize..]
            .iter()
            .any(|entry| *entry != LeapSecondTransition::default())
        {
            return Err(TimeError::Reserved);
        }
        Ok(())
    }

    pub fn offset_at_day(&self, unix_day: i32) -> Result<i16, TimeError> {
        self.validate()?;
        let mut offset = self.initial_tai_minus_utc;
        for transition in &self.transitions[..self.count as usize] {
            if unix_day < transition.effective_unix_day {
                break;
            }
            offset = transition.tai_minus_utc_after;
        }
        Ok(offset)
    }

    pub fn is_positive_leap_day(&self, unix_day: i32) -> Result<bool, TimeError> {
        self.validate()?;
        Ok(self.transitions[..self.count as usize]
            .iter()
            .any(|entry| entry.effective_unix_day == unix_day + 1))
    }

    /// Returns absolute TAI Q16 seconds on the Unix-day arithmetic origin.
    pub fn utc_to_tai_q16(&self, utc: UtcInstant) -> Result<i64, TimeError> {
        if utc.second_of_day > 86_400 {
            return Err(TimeError::Second);
        }
        let leap_second = utc.second_of_day == 86_400;
        if leap_second && !self.is_positive_leap_day(utc.unix_day)? {
            return Err(TimeError::LeapSecond);
        }
        let offset = self.offset_at_day(utc.unix_day)? as i64;
        let whole = utc.unix_day as i64 * SECONDS_PER_DAY + utc.second_of_day as i64 + offset;
        Ok((whole << 16) + utc.subsecond_q16 as i64)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct EarthOrientationSample {
    pub unix_day: i32,
    /// UT1-UTC in Q24 seconds.
    pub dut1_q24: i32,
    /// Polar motion radians in Q30.
    pub xp_q30: i32,
    pub yp_q30: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeError {
    Table,
    Reserved,
    Coverage,
    Second,
    LeapSecond,
    NegativeElapsed,
    MissionDuration,
}

pub fn elapsed_tai(
    table: &LeapSecondTable,
    epoch: UtcInstant,
    instant: UtcInstant,
) -> Result<MissionTimeQ16, TimeError> {
    let epoch_q16 = table.utc_to_tai_q16(epoch)?;
    let instant_q16 = table.utc_to_tai_q16(instant)?;
    let elapsed = instant_q16
        .checked_sub(epoch_q16)
        .ok_or(TimeError::NegativeElapsed)?;
    if elapsed < 0 {
        return Err(TimeError::NegativeElapsed);
    }
    if elapsed > u32::MAX as i64 {
        return Err(TimeError::MissionDuration);
    }
    MissionTimeQ16::from_raw(elapsed as u32).ok_or(TimeError::MissionDuration)
}

pub fn tai_to_tt_q16(tai_q16: i64) -> Result<i64, TimeError> {
    tai_q16
        .checked_add(TT_MINUS_TAI_Q16)
        .ok_or(TimeError::MissionDuration)
}

pub fn utc_to_ut1_q16(utc_q16: i64, dut1_q24: i32) -> Result<i64, TimeError> {
    // Q24 to Q16, ties away from zero.
    let correction = if dut1_q24 >= 0 {
        (dut1_q24 as i64 + 128) >> 8
    } else {
        (dut1_q24 as i64 - 128) >> 8
    };
    utc_q16
        .checked_add(correction)
        .ok_or(TimeError::MissionDuration)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leap_2016() -> LeapSecondTable {
        let mut table = LeapSecondTable::EMPTY;
        table.initial_tai_minus_utc = 36;
        table.count = 1;
        table.transitions[0] = LeapSecondTransition {
            effective_unix_day: 17_167, // 2017-01-01
            tai_minus_utc_after: 37,
        };
        table
    }

    #[test]
    fn utc_is_continuous_through_positive_leap_second() {
        let table = leap_2016();
        let before = table
            .utc_to_tai_q16(UtcInstant {
                unix_day: 17_166,
                second_of_day: 86_399,
                subsecond_q16: 0,
            })
            .unwrap();
        let leap = table
            .utc_to_tai_q16(UtcInstant {
                unix_day: 17_166,
                second_of_day: 86_400,
                subsecond_q16: 0,
            })
            .unwrap();
        let after = table
            .utc_to_tai_q16(UtcInstant {
                unix_day: 17_167,
                second_of_day: 0,
                subsecond_q16: 0,
            })
            .unwrap();
        assert_eq!(leap - before, 65_536);
        assert_eq!(after - leap, 65_536);
    }

    #[test]
    fn undeclared_leap_second_fails_closed() {
        let table = leap_2016();
        assert_eq!(
            table.utc_to_tai_q16(UtcInstant {
                unix_day: 17_165,
                second_of_day: 86_400,
                subsecond_q16: 0,
            }),
            Err(TimeError::LeapSecond)
        );
    }
}
