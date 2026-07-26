# Vendored host CLI dependencies

Phase 11.5 uses Clap only in the native host application. The dependency is
vendored because the accepted development environment builds offline and the
Windows TLS credential provider could not fetch the crates.io index during the
implementation run.

Pinned sources:

- `clap` 4.6.1, including `clap_builder` 4.6.0, `clap_derive` 4.6.1, and
  `clap_lex` 1.1.0, from upstream commit
  `ac5fda6a799e4c640d671edd1111d4a5e723dc1a`.
- `anstyle` 1.0.14 from upstream commit
  `0fe6f0ff6d52e9f91d4071199bd0b24bd46f3d35`.

Only the source required to compile the selected no-color Clap feature set is
retained. Upstream license files remain beside each vendored crate. The C64
crates do not depend on this directory.

An update requires a reviewed version change, a regenerated `Cargo.lock`, the
complete Phase 0–11 regression audit, and the Phase 11.5 CLI snapshot/parity
tests.
