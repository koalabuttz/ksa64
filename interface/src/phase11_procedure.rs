//! Strict Phase 11 deterministic procedure-pack contract (KPC11).

use crate::phase11::UplinkLoadType;
use crate::{crc32_ieee, CodecError};

pub const KPC11_LENGTH: usize = 4_096;
pub const KPC11_HEADER_LENGTH: usize = 128;
pub const KPC11_STEP_LENGTH: usize = 60;
pub const KPC11_MAX_STEPS: usize = 64;
const KPC11_CRC_OFFSET: usize = KPC11_LENGTH - 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ProcedureStepKind {
    Observe = 1,
    Verify = 2,
    Acknowledge = 3,
    Wait = 4,
    RequestAction = 5,
    Branch = 6,
    Caution = 7,
    Warning = 8,
    Complete = 9,
    Skip = 10,
    Fail = 11,
}

impl ProcedureStepKind {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::Observe),
            2 => Ok(Self::Verify),
            3 => Ok(Self::Acknowledge),
            4 => Ok(Self::Wait),
            5 => Ok(Self::RequestAction),
            6 => Ok(Self::Branch),
            7 => Ok(Self::Caution),
            8 => Ok(Self::Warning),
            9 => Ok(Self::Complete),
            10 => Ok(Self::Skip),
            11 => Ok(Self::Fail),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ProcedureComparison {
    Equal = 1,
    NotEqual = 2,
    Less = 3,
    LessOrEqual = 4,
    Greater = 5,
    GreaterOrEqual = 6,
    BitSet = 7,
    BitClear = 8,
}

impl ProcedureComparison {
    fn parse(value: u8) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::Equal),
            2 => Ok(Self::NotEqual),
            3 => Ok(Self::Less),
            4 => Ok(Self::LessOrEqual),
            5 => Ok(Self::Greater),
            6 => Ok(Self::GreaterOrEqual),
            7 => Ok(Self::BitSet),
            8 => Ok(Self::BitClear),
            _ => Err(CodecError::Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcedurePredicate {
    pub metric_id: u16,
    pub comparison: ProcedureComparison,
    pub flags: u8,
    pub threshold: i32,
}

impl ProcedurePredicate {
    pub const EMPTY: Self = Self {
        metric_id: 0,
        comparison: ProcedureComparison::Equal,
        flags: 0,
        threshold: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcedureStep {
    pub step_id: u16,
    pub kind: ProcedureStepKind,
    pub flags: u8,
    pub next_complete: u16,
    pub next_failure: u16,
    pub timeout_epochs: u32,
    pub action: Option<UplinkLoadType>,
    pub predicate_count: u8,
    pub hint_identity: u16,
    pub message_identity: u32,
    pub action_arguments: [i32; 2],
    pub predicates: [ProcedurePredicate; 4],
}

impl ProcedureStep {
    pub const EMPTY: Self = Self {
        step_id: 0,
        kind: ProcedureStepKind::Observe,
        flags: 0,
        next_complete: 0,
        next_failure: 0,
        timeout_epochs: 0,
        action: None,
        predicate_count: 0,
        hint_identity: 0,
        message_identity: 0,
        action_arguments: [0; 2],
        predicates: [ProcedurePredicate::EMPTY; 4],
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcedurePack {
    pub pack_identity: u32,
    pub plan_identity: u32,
    pub procedure_identity: u32,
    pub metric_catalog_identity: u32,
    pub name_identity: u32,
    pub step_count: u16,
    pub entry_step: u16,
    pub flags: u32,
    pub steps: [ProcedureStep; KPC11_MAX_STEPS],
}

pub fn write_kpc11(value: &ProcedurePack, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KPC11_LENGTH
        || value.pack_identity == 0
        || value.plan_identity == 0
        || value.procedure_identity == 0
        || value.metric_catalog_identity == 0
        || value.name_identity == 0
        || value.step_count == 0
        || usize::from(value.step_count) > KPC11_MAX_STEPS
        || value.entry_step == 0
        || value.flags & !3 != 0
    {
        return Err(CodecError::Flags);
    }
    output.fill(0);
    output[..4].copy_from_slice(b"KPC1");
    output[4] = 11;
    p16(output, 6, KPC11_LENGTH as u16);
    p32(output, 8, value.pack_identity);
    p32(output, 12, value.plan_identity);
    p32(output, 16, value.procedure_identity);
    p32(output, 20, value.metric_catalog_identity);
    p32(output, 24, value.name_identity);
    p16(output, 28, value.step_count);
    p16(output, 30, value.entry_step);
    p32(output, 32, value.flags);
    for index in 0..usize::from(value.step_count) {
        write_step(
            &value.steps[index],
            &mut output[KPC11_HEADER_LENGTH + index * KPC11_STEP_LENGTH
                ..KPC11_HEADER_LENGTH + (index + 1) * KPC11_STEP_LENGTH],
        )?;
    }
    p32(
        output,
        KPC11_CRC_OFFSET,
        crc32_ieee(&output[..KPC11_CRC_OFFSET]),
    );
    Ok(())
}

pub fn parse_kpc11(input: &[u8]) -> Result<ProcedurePack, CodecError> {
    if input.len() != KPC11_LENGTH {
        return Err(CodecError::Length);
    }
    if input[..4] != *b"KPC1" || input[4] != 11 || g16(input, 6) != KPC11_LENGTH as u16 {
        return Err(CodecError::Enum);
    }
    if crc32_ieee(&input[..KPC11_CRC_OFFSET]) != g32(input, KPC11_CRC_OFFSET) {
        return Err(CodecError::Checksum);
    }
    if input[5] != 0 || input[36..KPC11_HEADER_LENGTH].iter().any(|byte| *byte != 0) {
        return Err(CodecError::Reserved);
    }
    let step_count = g16(input, 28);
    let used_end = KPC11_HEADER_LENGTH + usize::from(step_count) * KPC11_STEP_LENGTH;
    if step_count == 0
        || usize::from(step_count) > KPC11_MAX_STEPS
        || used_end > KPC11_CRC_OFFSET
        || input[used_end..KPC11_CRC_OFFSET]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(CodecError::Reserved);
    }
    let mut steps = [ProcedureStep::EMPTY; KPC11_MAX_STEPS];
    for (index, step) in steps.iter_mut().take(usize::from(step_count)).enumerate() {
        *step = parse_step(
            &input[KPC11_HEADER_LENGTH + index * KPC11_STEP_LENGTH
                ..KPC11_HEADER_LENGTH + (index + 1) * KPC11_STEP_LENGTH],
        )?;
    }
    let value = ProcedurePack {
        pack_identity: g32(input, 8),
        plan_identity: g32(input, 12),
        procedure_identity: g32(input, 16),
        metric_catalog_identity: g32(input, 20),
        name_identity: g32(input, 24),
        step_count,
        entry_step: g16(input, 30),
        flags: g32(input, 32),
        steps,
    };
    if value.pack_identity == 0
        || value.plan_identity == 0
        || value.procedure_identity == 0
        || value.metric_catalog_identity == 0
        || value.name_identity == 0
        || value.entry_step == 0
        || value.flags & !3 != 0
        || !value.steps[..usize::from(value.step_count)]
            .iter()
            .any(|step| step.step_id == value.entry_step)
    {
        return Err(CodecError::Flags);
    }
    Ok(value)
}

fn write_step(value: &ProcedureStep, output: &mut [u8]) -> Result<(), CodecError> {
    if output.len() != KPC11_STEP_LENGTH
        || value.step_id == 0
        || value.flags & !7 != 0
        || value.predicate_count > 4
        || value.message_identity == 0
    {
        return Err(CodecError::Flags);
    }
    output.fill(0);
    p16(output, 0, value.step_id);
    output[2] = value.kind as u8;
    output[3] = value.flags;
    p16(output, 4, value.next_complete);
    p16(output, 6, value.next_failure);
    p32(output, 8, value.timeout_epochs);
    output[12] = value.action.map_or(0, |action| action as u8);
    output[13] = value.predicate_count;
    p16(output, 14, value.hint_identity);
    p32(output, 16, value.message_identity);
    p32(output, 20, value.action_arguments[0] as u32);
    p32(output, 24, value.action_arguments[1] as u32);
    for index in 0..usize::from(value.predicate_count) {
        let offset = 28 + index * 8;
        let predicate = value.predicates[index];
        if predicate.metric_id == 0 || predicate.flags & !1 != 0 {
            return Err(CodecError::Flags);
        }
        p16(output, offset, predicate.metric_id);
        output[offset + 2] = predicate.comparison as u8;
        output[offset + 3] = predicate.flags;
        p32(output, offset + 4, predicate.threshold as u32);
    }
    Ok(())
}

fn parse_step(input: &[u8]) -> Result<ProcedureStep, CodecError> {
    if input.len() != KPC11_STEP_LENGTH {
        return Err(CodecError::Length);
    }
    let predicate_count = input[13];
    if predicate_count > 4 {
        return Err(CodecError::Flags);
    }
    let action = match input[12] {
        0 => None,
        1 => Some(UplinkLoadType::GroundNavigationUpdate),
        2 => Some(UplinkLoadType::MissionEventTarget),
        3 => Some(UplinkLoadType::ContingencyBranch),
        4 => Some(UplinkLoadType::NavigationMode),
        5 => Some(UplinkLoadType::HighLevelMode),
        _ => return Err(CodecError::Enum),
    };
    let mut predicates = [ProcedurePredicate::EMPTY; 4];
    for (index, predicate) in predicates
        .iter_mut()
        .take(usize::from(predicate_count))
        .enumerate()
    {
        let offset = 28 + index * 8;
        *predicate = ProcedurePredicate {
            metric_id: g16(input, offset),
            comparison: ProcedureComparison::parse(input[offset + 2])?,
            flags: input[offset + 3],
            threshold: g32(input, offset + 4) as i32,
        };
        if predicate.metric_id == 0 || predicate.flags & !1 != 0 {
            return Err(CodecError::Flags);
        }
    }
    if input[28 + usize::from(predicate_count) * 8..]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(CodecError::Reserved);
    }
    let value = ProcedureStep {
        step_id: g16(input, 0),
        kind: ProcedureStepKind::parse(input[2])?,
        flags: input[3],
        next_complete: g16(input, 4),
        next_failure: g16(input, 6),
        timeout_epochs: g32(input, 8),
        action,
        predicate_count,
        hint_identity: g16(input, 14),
        message_identity: g32(input, 16),
        action_arguments: [g32(input, 20) as i32, g32(input, 24) as i32],
        predicates,
    };
    if value.step_id == 0 || value.flags & !7 != 0 || value.message_identity == 0 {
        return Err(CodecError::Flags);
    }
    Ok(value)
}

fn p16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn p32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn g16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}

fn g32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> ProcedurePack {
        let mut steps = [ProcedureStep::EMPTY; KPC11_MAX_STEPS];
        steps[0] = ProcedureStep {
            step_id: 1,
            kind: ProcedureStepKind::Verify,
            flags: 0,
            next_complete: 2,
            next_failure: 3,
            timeout_epochs: 64,
            action: None,
            predicate_count: 1,
            hint_identity: 9,
            message_identity: 0x1100_0001,
            action_arguments: [0; 2],
            predicates: [
                ProcedurePredicate {
                    metric_id: 1,
                    comparison: ProcedureComparison::Equal,
                    flags: 0,
                    threshold: 1,
                },
                ProcedurePredicate::EMPTY,
                ProcedurePredicate::EMPTY,
                ProcedurePredicate::EMPTY,
            ],
        };
        steps[1] = ProcedureStep {
            step_id: 2,
            kind: ProcedureStepKind::Complete,
            message_identity: 0x1100_0002,
            ..ProcedureStep::EMPTY
        };
        steps[2] = ProcedureStep {
            step_id: 3,
            kind: ProcedureStepKind::Fail,
            message_identity: 0x1100_0003,
            ..ProcedureStep::EMPTY
        };
        ProcedurePack {
            pack_identity: 0x11c2_0001,
            plan_identity: 0x11a0_0001,
            procedure_identity: 0x11c2_1001,
            metric_catalog_identity: 0x11c2_2001,
            name_identity: 0x11c2_3001,
            step_count: 3,
            entry_step: 1,
            flags: 0,
            steps,
        }
    }

    #[test]
    fn kpc11_round_trips_and_rejects_reserved_or_corrupt_data() {
        let value = pack();
        let mut bytes = [0; KPC11_LENGTH];
        write_kpc11(&value, &mut bytes).unwrap();
        assert_eq!(parse_kpc11(&bytes).unwrap(), value);
        bytes[100] = 1;
        let crc = crc32_ieee(&bytes[..KPC11_CRC_OFFSET]);
        p32(&mut bytes, KPC11_CRC_OFFSET, crc);
        assert_eq!(parse_kpc11(&bytes), Err(CodecError::Reserved));
        write_kpc11(&value, &mut bytes).unwrap();
        bytes[200] ^= 1;
        assert_eq!(parse_kpc11(&bytes), Err(CodecError::Checksum));
    }
}
