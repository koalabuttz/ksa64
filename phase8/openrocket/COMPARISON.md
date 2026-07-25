# Phase 8 OpenRocket comparison

OpenRocket 24.12 was run headlessly with the checked Java harness. The `.ork`, native CSV exports, settings, and hashes are retained beside this report.

| Check | KSA64 | OpenRocket | Error | Limit | Result |
|---|---:|---:|---:|---:|:---:|
| calm.rail_exit_velocity_mps | 19.1638 | 19.0346 | 0.6787% | 5% | PASS |
| calm.max_velocity_mps | 139.255 | 137.342 | 1.393% | 5% | PASS |
| calm.apogee_m | 754.234 | 745.315 | 1.197% | 5% | PASS |
| calm.burnout_time_s | 2.32961 | 2.324 | 0.2413% | 5% | PASS |
| calm.max_dynamic_pressure_pa | 11704.3 | 11389.6 | 2.764% | 10% | PASS |
| calm.time_to_apogee_s | 12.0498 | 11.9898 | 0.06002 | 0.5 | PASS |
| calm.max_aoa_deg | 0.0022546 | 0.060804 | 0.05855 | 2 | PASS |
| crosswind5.rail_exit_velocity_mps | 19.1623 | 19.0273 | 0.7094% | 5% | PASS |
| crosswind5.max_velocity_mps | 138.92 | 136.853 | 1.51% | 5% | PASS |
| crosswind5.apogee_m | 742.22 | 728.557 | 1.875% | 5% | PASS |
| crosswind5.burnout_time_s | 2.32961 | 2.324 | 0.2413% | 5% | PASS |
| crosswind5.max_dynamic_pressure_pa | 11767.7 | 11311 | 4.038% | 10% | PASS |
| crosswind5.time_to_apogee_s | 11.9698 | 11.8698 | 0.1 | 0.5 | PASS |
| crosswind5.max_aoa_deg | 14.6126 | 14.6933 | 0.08076 | 2 | PASS |
| crosswind5.landing_distance_m | 234.672 | 232.098 | 2.574 | 50 | PASS |
| geometry.dry_mass_kg | 2.11204 | 2.11204 | 0% | 0.5% | PASS |
| geometry.cp_caliber_delta | 1.56692 | 1.56762 | 0.0007087 | 0.01441 | PASS |
| geometry.cg_caliber_delta | 0.972606 | 0.973546 | 0.0009398 | 0.01441 | PASS |
| geometry.static_margin_calibers | 10.3075 | 10.3035 | 0.004008 | 0.25 | PASS |

Weathercocking and recovery drift agree after the documented OpenRocket-to-ENU axis mapping.

The original Phase 7 axial-Cd placeholder was replaced by rounded effective coefficients from the aligned OpenRocket geometry before KSA64 was rerun. The near-apogee AoA singularity is explicitly outside the directional model below 50 Pa; state propagation continues.

This is engineering evidence, not launch approval, certification, regulatory guidance, or safety authority.
