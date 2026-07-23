# Phase 2 deterministic C64 replay

Status: accepted.

The canonical 57,704-byte `KST2` stream remains the regression truth. A generated 2,851-byte `KRP2` presentation tape derives one screen coordinate and the accepted event byte from each of its 901 frames, then embeds the original canonical header and terminal frame. The tape binds itself to source-stream CRC `0x7d13b2bf`, scenario identity, terminal checksum `0xcc57612b`, full-mission Max-Q, and exact fixed-point orbit; it carries header and whole-tape CRCs and is frozen by SHA-256 `35db68c6b8fc602b1ce760552f2b2994b2ec66a27df6b02acba8cce66774ed08`.

`KRP2` is not a second physics or telemetry format. It is a compact display index generated only after the host validates canonical KST2. The C64 still decodes and binds the embedded canonical KST2 header and terminal frame through the portable core contract. A generated table-driven CRC implementation makes the cold-path tape check practical on the 6510 without changing canonical CRC results.

The 16,169-byte replay PRG is verified directly from actual 40x25 screen memory under PAL VICE. It draws a 50-cell altitude/downrange arc and reports the 188.169 x 188.169 km orbit, full-run Max-Q 40.779 kPa, 901 frames, final state, and the `cc57612b` checksum. Its constant-memory retained sink occupies 135 bytes.

Replay drives bounded SID voice-one cues in deterministic event order: one ignition, two cutoffs, one separation, and one end cue. The nominal schedule hash is `0x9473fcdb`; impact uses a distinct alarm waveform and is absent from the nominal run. Presentation and SID work are explicitly excluded from physics timing.
