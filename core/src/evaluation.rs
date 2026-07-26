//! Profile-neutral evaluation contracts introduced by Phase 7.
//!
//! Raw values are meaningful only with the selected [`ModelProfileId`]. A
//! validity bitmap keeps unsupported or unavailable metrics distinct from a
//! legitimate numeric zero without requiring floating point or allocation.

pub const EVALUATION_V1_METRIC_COUNT: usize = 24;
pub const EVALUATION_METRIC_COUNT: usize = 32;
pub const EVALUATION_IDENTITY_COUNT: usize = 6;
pub const EVALUATION_CHECKSUM_COUNT: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ModelProfileId {
    LegacyKsa2PlanarV1 = 1,
    LegacyKsa5SpatialV1 = 2,
    HobbyVerticalV1 = 3,
    HobbySpatialV1 = 4,
    GlobalEcef6DofV1 = 5,
}

impl ModelProfileId {
    /// Canonical scale-neutral name for the accepted Phase 7 profile.
    #[allow(non_upper_case_globals)]
    pub const VerticalPointMassV1: Self = Self::HobbyVerticalV1;

    /// Canonical frame/model name for the accepted Phase 8 profile.
    #[allow(non_upper_case_globals)]
    pub const LocalEnu6DofV1: Self = Self::HobbySpatialV1;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EvaluationOutcome {
    Complete = 0,
    StableOrbit = 1,
    CompleteNotOrbit = 2,
    GroundContact = 3,
    Aborted = 4,
    NumericFault = 5,
    StepLimit = 6,
    NoLiftoff = 7,
    ConfigurationFault = 8,
    RecoveryIncomplete = 9,
    ModelEnvelopeExceeded = 10,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MetricSlot {
    PerigeeAltitude = 0,
    ApogeeAltitude = 1,
    Inclination = 2,
    MaxDynamicPressure = 3,
    MaxAcceleration = 4,
    MaxSpeed = 5,
    MaxMach = 6,
    RailExitTime = 7,
    RailExitVelocity = 8,
    BurnoutTime = 9,
    BurnoutAltitude = 10,
    BurnoutVelocity = 11,
    ApogeeTime = 12,
    DrogueTime = 13,
    DrogueAltitude = 14,
    DrogueVelocity = 15,
    MainTime = 16,
    MainAltitude = 17,
    MainVelocity = 18,
    MaxOpeningDeceleration = 19,
    GroundContactTime = 20,
    ImpactVelocity = 21,
    MaxNavigationError = 22,
    TerminalMass = 23,
    MinimumStaticMargin = 24,
    MaximumAngleOfAttack = 25,
    MaximumAngularRate = 26,
    MaximumLateralAcceleration = 27,
    LandingDistance = 28,
    RailExitStaticMargin = 29,
    BurnoutStaticMargin = 30,
    MaximumWindSpeed = 31,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct MetricValidity(u32);

impl MetricValidity {
    pub const NONE: Self = Self(0);

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, slot: MetricSlot) -> bool {
        self.0 & (1u32 << slot as u8) != 0
    }

    pub fn insert(&mut self, slot: MetricSlot) {
        self.0 |= 1u32 << slot as u8;
    }
}

/// Fixed-size result shared by all Phase 7 evaluators.
///
/// `terminal_state_a`, `terminal_state_b`, and `metrics` deliberately contain
/// profile-native raw integers. Consumers must use `profile` and
/// `metric_validity` before interpreting them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct EvaluationSummary {
    pub profile: ModelProfileId,
    pub outcome: EvaluationOutcome,
    pub numeric_faults: u8,
    pub steps: u32,
    pub metric_validity: MetricValidity,
    pub terminal_state_a: [i32; 3],
    pub terminal_state_b: [i32; 3],
    pub metrics: [i32; EVALUATION_METRIC_COUNT],
    pub events: u32,
    pub identities: [u32; EVALUATION_IDENTITY_COUNT],
    pub source_checksums: [u32; EVALUATION_CHECKSUM_COUNT],
}

impl EvaluationSummary {
    pub const fn empty(profile: ModelProfileId) -> Self {
        Self {
            profile,
            outcome: EvaluationOutcome::Complete,
            numeric_faults: 0,
            steps: 0,
            metric_validity: MetricValidity::NONE,
            terminal_state_a: [0; 3],
            terminal_state_b: [0; 3],
            metrics: [0; EVALUATION_METRIC_COUNT],
            events: 0,
            identities: [0; EVALUATION_IDENTITY_COUNT],
            source_checksums: [0; EVALUATION_CHECKSUM_COUNT],
        }
    }

    pub fn set_metric(&mut self, slot: MetricSlot, raw: i32) {
        self.metrics[slot as usize] = raw;
        self.metric_validity.insert(slot);
    }

    pub const fn metric(self, slot: MetricSlot) -> Option<i32> {
        if self.metric_validity.contains(slot) {
            Some(self.metrics[slot as usize])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_validity_distinguishes_zero_from_absent() {
        let mut summary = EvaluationSummary::empty(ModelProfileId::HobbyVerticalV1);
        assert_eq!(summary.metric(MetricSlot::ApogeeAltitude), None);
        summary.set_metric(MetricSlot::ApogeeAltitude, 0);
        assert_eq!(summary.metric(MetricSlot::ApogeeAltitude), Some(0));
    }

    #[test]
    fn reserved_validity_bits_are_discarded() {
        let validity = MetricValidity::from_bits(u32::MAX);
        assert_eq!(validity.bits(), u32::MAX);
    }

    #[test]
    fn phase8_uses_the_upper_metric_word_without_moving_phase7_slots() {
        assert_eq!(
            MetricSlot::TerminalMass as usize,
            EVALUATION_V1_METRIC_COUNT - 1
        );
        assert_eq!(
            MetricSlot::MinimumStaticMargin as usize,
            EVALUATION_V1_METRIC_COUNT
        );
        assert_eq!(
            MetricSlot::MaximumWindSpeed as usize,
            EVALUATION_METRIC_COUNT - 1
        );
    }

    #[test]
    fn canonical_profile_aliases_preserve_frozen_wire_identities() {
        assert_eq!(
            ModelProfileId::VerticalPointMassV1 as u8,
            ModelProfileId::HobbyVerticalV1 as u8
        );
        assert_eq!(
            ModelProfileId::LocalEnu6DofV1 as u8,
            ModelProfileId::HobbySpatialV1 as u8
        );
    }
}
