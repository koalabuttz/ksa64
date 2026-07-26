# Phase 11.5 executable inventory

This inventory classifies the executable surface at the Phase 11 baseline.
Names remain available; classification determines whether the unified product
catalog presents them directly.

## Current host product entrypoints

- Mission and operations: `phase6_launch`, `phase8_5_launch`,
  `phase9_5_launch`, `phase10_launch`, `phase11_mission_control`, `phase11`.
- Mission compilation and execution: `phase7_compile`, `phase7_run`,
  `phase8_compile`, `phase8_run`, `phase8_artifacts`, `phase9_5_compile`.
- Campaign and workbench: `phase7_campaign`, `phase8_campaign`,
  `phase8_5_campaign`, `phase9`, `phase9_5_workbench`, `phase10_campaign`.
- Evidence and replay: `phase6_session`, `phase8_export`,
  `phase9_5_finalists`, `phase10_world_reference`.
- Split placement: `phase6_bridge`, `phase8_5_bridge`, `phase9_5_bridge`,
  `phase9_5_finalist_bridge`, `phase10_bridge`.

These become application-service adapters and compatibility entrypoints.

## Host audit and reference utilities

- `phase4_campaign`, `phase4_export`, `phase4_join`, `phase4_stock_pack`
- `phase5_campaign`, `phase5_history`, `phase5_telemetry`
- `phase7_trace`, `phase8_trace_host`
- `phase9_5_integrated`
- `phase11_reference_ops_fixture`

These stay phase-named and appear only in the historical catalog or audits.

## Stock-C64 product and replay targets

- `core/phase7_full_c64`, `core/phase7_replay_c64`
- `core/phase8_full_c64`, `core/phase8_replay_c64`
- `core/phase9_finalist_c64`, `core/phase9_5_finalist_c64`
- `core/phase10_replay_c64`
- `sim/phase6_flight_endpoint_c64`
- `sim/phase8_5_mailbox_endpoint_c64`
- `sim/phase9_5_flight_endpoint_c64`
- `sim/phase9_5_finalist_flight_endpoint_c64`
- `sim/phase10_flight_endpoint_c64`
- `sim/phase11_safehold_endpoint_c64`
- `sim/phase11_reference_ops_endpoint_c64`

These receive domain-oriented target descriptors. Their Cargo binary names and
bytes remain unchanged.

## C64 timing, contract, and finite-probe targets

- `core/acceptance_c64`, `core/numeric_c64`, `core/numeric_sim`
- `core/phase2_contract_sim`, `core/phase2_mission_sim`,
  `core/phase2_replay_c64`, `core/phase2_timed_c64`,
  `core/phase2_vacuum_timed_c64`
- `core/phase5_flexible_sim`, `core/phase5_rigid_asymmetric_sim`,
  `core/phase5_rigid_sim`, `core/phase5_rigid_spherical_sim`,
  `core/phase5_spatial_sim`, `core/phase5_world_sim`
- `core/phase7_trace_c64`, `core/phase8_contract_c64`,
  `core/phase8_trace_c64`, `core/phase9_5_canard_c64`,
  `core/phase9_5_rcs_c64`, `core/production_timed_c64`,
  `core/telemetry_status_c64`, `core/telemetry_timed_c64`
- `sim/phase3_probes_c64`, `sim/phase3_replay_c64`
- `sim/phase4_campaign_sim`, `sim/phase4_distribution_sim`,
  `sim/phase4_export_c64`, `sim/phase4_reu_probe_c64`,
  `sim/phase4_stock_c64`
- `sim/phase5_avionics_sim`, `sim/phase5_avionics_timed_c64`,
  `sim/phase5_campaign_sim`, `sim/phase5_guidance_sim`,
  `sim/phase5_history_reu_c64`, `sim/phase5_history_sim`,
  `sim/phase5_replay_c64`, `sim/phase5_telemetry_sim`,
  `sim/phase5_telemetry_timed_c64`, `sim/phase5_vehicle_sim`,
  `sim/phase5_vehicle_timed_c64`
- `sim/phase6_endpoint_probe_c64`, `sim/phase6_mailbox_endpoint_c64`,
  `sim/phase6_realtime_timed_c64`
- `sim/phase8_5_avionics_timed_c64`, `sim/phase8_5_combined_c64`
- `sim/phase9_5_advanced_timed_c64`, `sim/phase9_5_allocator_probe_c64`,
  `sim/phase9_5_avionics_probe_c64`, `sim/phase9_5_avionics_timed_c64`,
  `sim/phase9_5_contract_c64`
- `sim/phase10_flight_timed_c64`
- `sim/phase11_safehold_probe_c64`, `sim/phase11_safehold_reference`

These remain internal verification tools. They are discoverable only through
the historical tier, target evidence, or phase audit that owns them.
