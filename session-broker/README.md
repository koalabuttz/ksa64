# KSA64 Session Broker

This crate is the transport-neutral security and authority foundation for Phase 12B.5.
It does **not** own or reproduce simulation physics.

## Boundaries

- One bounded worker thread owns one BrokerAuthority implementation.
- The authority continues autonomous fast or real-time advancement when a presentation client disconnects.
- A session has one immutable presentation role and at most one controlling client lease.
- Reconnect uses the session nonce plus independent KPS1 cursors. A retention gap returns ResyncRequired.
- Clients submit only PresentationActionIntent values; the authority adapter constructs and validates canonical Phase 11 records.

ksa64-session integration is exposed behind the portable-session feature while the concrete
role-filtering adapter lives with the portable session crate. This keeps the security worker usable
for replay and test authorities without introducing a second simulator.

## Browser admission

`LoopbackWebService` is the production loopback TCP/WebSocket boundary. It serves an optional bounded embedded PWA shell and routes live binary KPS1 traffic through `BrowserAdmissionController`:

- loopback-only bind validation;
- exact Origin allowlists;
- 256-bit per-launch tokens carried only in the WebSocket subprotocol;
- binary KPS1 records only;
- connection, outstanding-command, and caller-clocked rate bounds.

Accepted sockets are switched back to blocking mode explicitly, carry finite HTTP/WebSocket timeouts and message-size limits, and are bounded before a reader thread is spawned. The listener rejects wildcard binds, query-string tokens, text application messages, stale sequence/session identities, and retention gaps; stale cursors receive a typed `ResyncRequired` response. Presentation disconnect never pauses the authority.


## Runnable loopback Mission Control

After building the PWA in `web/dist`, start the accepted guided GNSS-loss authority and serve that build from one loopback-only process:

```text
cargo run -p ksa64-session-broker --features launcher --bin ksa64-session-broker
```

The launcher prints the exact HTTP URL, allowed Origin, WebSocket endpoint, and session nonce. The per-launch subprotocol credential is injected only into the uncached runtime configuration and is never printed. It loads the bounded PWA asset set into memory, replaces only the uncached `/runtime-config.js` response with the current loopback endpoint and bare browser token, leaves `index.html` token-free, and waits for Enter before shutting the socket service and authority worker down cleanly. For automation, `--run-for-seconds N` provides a bounded lifetime. `--port` must be nonzero because Origin matching is exact.

The launcher never starts a LAN listener. Paired LAN is an explicit application choice through `PairedLanService`; callers must also persist the strict bounded peer-registry bytes returned by `PeerRegistry::export_bounded` and protect the server's static Noise identity.

## Paired LAN transport

`PairedLanService` is the opt-in native/Vita TCP boundary. It binds only the explicitly selected private or link-local interface, accepts one connection, exposes pending first-pairing details for local confirmation, assigns broker identities from authenticated peer keys rather than client claims, and dispatches encrypted role-filtered KPS1 traffic. The native/Vita path uses Snow 0.10 with:

- Noise_XX_25519_ChaChaPoly_BLAKE2s for first pairing;
- a six-digit comparison code derived from the handshake hash and confirmed locally on both devices;
- stored peer public keys with immutable role binding and revocation;
- Noise_IK_25519_ChaChaPoly_BLAKE2s for authenticated reconnects;
- length-prefixed, authenticated fragments carrying strict KPS1 frames.

KPS1 records can exceed Noise's single-message size. The channel therefore fragments records only
after validating them, enforces ordered non-interleaved assembly, validates the complete KPS1 record
before publication, and permanently poisons a channel after malformed encrypted traffic. Completed sealed evidence is verified against its advertised length and CRC before it is emitted in bounded KPS1 chunks.

The cryptographic transport itself is `no_std + alloc`. Constrained clients provision a 32-byte handshake entropy seed; native builds source that seed from the operating system. The no-default dependency graph does not pull an operating-system entropy backend.

This crate deliberately contains no discovery, UPnP, NAT traversal, wildcard listener, Internet
service, account system, or custom cryptography.


## Explicit host paired-LAN launcher

`ksa64-paired-lan` is deliberately separate from the loopback browser launcher. It starts the
accepted guided GNSS-loss authority for one native/Vita **Guided Operator** peer over a
user-selected private or link-local address. It refuses wildcard, loopback, multicast, public,
discovery, UPnP, NAT-traversal, and Internet-facing binds.

```text
cargo run -p ksa64-session-broker --features launcher --bin ksa64-paired-lan -- \
  serve --bind 192.168.1.42:27864 --state-dir ./local-paired-lan \
  --session-nonce 4b53413600000001
```

The state directory is deliberately user-selected and must be protected as local private
configuration. It contains a CRC-protected server Noise identity (`paired-server.ksk1`) and a
CRC-protected peer registry (`paired-peers.ppr1`); neither is canonical evidence, a session
secret, a KSB11 bundle, or content for source control. Writes use a synced temporary file and a
recoverable backup. A corrupted present file is rejected rather than silently trusted.

On first connection, use `pending` at the local host and compare the displayed six-digit code on
both devices. Only then enter `confirm PAIRING_ID SIX_DIGIT_CODE`; a new peer is persisted with
its immutable Guided Operator role. `reject`, `peers`, and `revoke PUBLIC_KEY_HEX` are local host
commands. Revocation is persisted and disconnects the peer on its next bounded transport poll;
future Noise IK reconnects reject it immediately.

```text
ksa64-paired-lan list --state-dir ./local-paired-lan
ksa64-paired-lan revoke --state-dir ./local-paired-lan --public-key 64_HEX_CHARACTERS
```

This is a host engineering interface, not physical Vita acceptance. The actual Vita3K and
physical-device pairing, controls, reconnect, suspend/resume, memory, and timing gates remain
pending.
