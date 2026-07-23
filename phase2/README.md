# Phase 2: planar multistage ascent

Status: numeric contract, vacuum geometry, atmosphere, aerodynamics, and open-loop guidance accepted; bounded staging and complete missions are next.

Phase 2 extends the accepted vertical laboratory into an equatorial, rotating-Earth, multistage launch and orbital-insertion simulation. Its integrated demonstration is a fictional two-stage KSA-2A mission targeting a 180–220 km near-circular orbit, plus a deterministic failed-insertion variant.

The implementation is intentionally layered: numeric/data contract, vacuum coast and integrator selection, atmosphere/aerodynamics/guidance, staging, telemetry and independent validation, then deterministic C64 replay. Every layer retains exact host/MOS comparison and target-visible timing.
