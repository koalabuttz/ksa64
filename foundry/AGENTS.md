# Mission Foundry working rules

Mission Foundry is a presentation and authoring client of the accepted KSA64
application. Rust remains the only authority for simulation, operations,
physical state, canonical evidence, and role filtering.

- Graphical clients operate live missions only through `LiveMissionSession`.
  They must not create an execution loop or reconstruct a live mission from a
  completed replay.
- The viewer bridge filters data by immutable session role before data crosses
  into Unreal. Never expose SIM Director truth to other roles.
- Unreal C++, Blueprint, UMG, editor Python, and MCP are consumers and tooling;
  they may not add substitute physics, new canonical formats, or an unrecorded
  control path.
- MCP is optional, loopback-only supervised editor tooling. Builds, tests,
  packaging, and packaged runtime must work with MCP and Python disabled.
- Track authored `.uasset`/`.umap` files and asset masters through Git LFS.
  Do not commit generated engine output. Every imported asset needs provenance,
  source hash, rights/credit information, transformations, and an explicit
  `engineering_authority: false` declaration where it is not KSA64 geometry.
- Keep the runtime bridge asynchronous and presentation-only. Validate ABI and
  staged DLL hashes before load; never run Cargo from UnrealBuildTool.
- Verify native Rust gates and Unreal automation/package smoke tests before
  accepting a Foundry change.