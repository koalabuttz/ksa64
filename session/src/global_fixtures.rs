//! Browser-safe embedded Phase 10 fixture identities used by live sessions.

use ksa64_core::phase10_contract::{EarthModelPack, TransformPack};
use ksa64_core::phase10_environment::CompiledAtmospherePack;
use ksa64_core::phase10_vehicle::{GlobalMissionPack, GlobalVehiclePack};

#[derive(Clone)]
pub struct GlobalFixtureSet {
    pub earth: EarthModelPack,
    pub transforms: TransformPack,
    pub atmosphere: CompiledAtmospherePack,
    pub vehicle: GlobalVehiclePack,
    pub mission: GlobalMissionPack,
}

impl GlobalFixtureSet {
    pub fn embedded() -> Self {
        Self {
            earth: EarthModelPack::decode(include_bytes!("../../phase10/generated/ksa-g10r.kem10"))
                .expect("embedded KEM10"),
            transforms: TransformPack::decode(include_bytes!(
                "../../phase10/generated/ksa-g10r.kft10"
            ))
            .expect("embedded KFT10"),
            atmosphere: CompiledAtmospherePack::decode(include_bytes!(
                "../../phase10/generated/ksa-g10r.kat10"
            ))
            .expect("embedded KAT10"),
            vehicle: GlobalVehiclePack::decode(include_bytes!(
                "../../phase10/generated/ksa-g10r.kgv10"
            ))
            .expect("embedded KGV10"),
            mission: GlobalMissionPack::decode(include_bytes!(
                "../../phase10/generated/ksa-g10r.kgm10"
            ))
            .expect("embedded KGM10"),
        }
    }
}
