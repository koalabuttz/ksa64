//! Frozen fixed-capacity Phase 4 record sizes and identities.

pub const MAX_DISTRIBUTIONS: usize = 16;
pub const CAMPAIGN_CONFIG_LENGTH: usize = 512;
pub const DISTRIBUTION_RECORD_LENGTH: usize = 24;
pub const RUN_SUMMARY_LENGTH: usize = 128;
pub const PLOT_HEADER_LENGTH: usize = 64;
pub const PLOT_POINT_LENGTH: usize = 8;
pub const DETAIL_HEADER_LENGTH: usize = 96;
pub const DETAIL_FRAME_LENGTH: usize = crate::telemetry::PHASE3_TELEMETRY_FRAME_LENGTH;
pub const ARCHIVE_SUPERBLOCK_LENGTH: usize = 256;
pub const ARCHIVE_RECORD_HEADER_LENGTH: usize = 32;
pub const EXPORT_VOLUME_HEADER_LENGTH: usize = 64;
pub const STOCK_PLOT_STRIDE: u32 = 32;
pub const REU_PLOT_STRIDE: u32 = 8;
pub const STOCK_INTERESTING_SUMMARIES: usize = 5;
pub const SMOKE_RUNS: u32 = 64;
pub const REFERENCE_RUNS: u32 = 1_024;
pub const REFERENCE_MASTER_SEED: u32 = 0x4b53_4134;

pub const KSC4_MAGIC: [u8; 4] = *b"KSC4";
pub const KSR4_MAGIC: [u8; 4] = *b"KSR4";
pub const KPH4_MAGIC: [u8; 4] = *b"KPH4";
pub const KST4_MAGIC: [u8; 4] = *b"KST4";
pub const KRA4_MAGIC: [u8; 4] = *b"KRA4";
pub const KXV4_MAGIC: [u8; 4] = *b"KXV4";

const _: () =
    assert!(128 + MAX_DISTRIBUTIONS * DISTRIBUTION_RECORD_LENGTH == CAMPAIGN_CONFIG_LENGTH);
const _: () = assert!(DETAIL_FRAME_LENGTH == 160);
