//! Explicit native/Vita paired-LAN authority launcher.
//!
//! This binary intentionally does not serve the browser PWA. It binds a single
//! user-selected private interface and requires a local comparison-code command
//! before a first-paired client receives an authenticated role.

use std::{
    env,
    io::{self, BufRead, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use ksa64_interface::phase11::OperationalRole;
use ksa64_presentation::PresentationRole;
use ksa64_session::presentation_adapter::FullMissionPresentationSession;
use ksa64_session_broker::{
    load_or_create_server_identity, load_peer_registry, save_peer_registry, ComparisonCode,
    PairedLanConfig, PairedLanService, PortableFullMissionAuthority, SessionBrokerHandle,
    WorkerConfig,
};

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Serve {
        bind: SocketAddr,
        state_dir: PathBuf,
        session_nonce: u64,
    },
    List {
        state_dir: PathBuf,
    },
    Revoke {
        state_dir: PathBuf,
        public_key: [u8; 32],
    },
    Help,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("KSA64 paired-LAN error: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    match parse_command(env::args().skip(1))? {
        Command::Serve {
            bind,
            state_dir,
            session_nonce,
        } => serve(bind, state_dir, session_nonce),
        Command::List { state_dir } => list_peers(state_dir),
        Command::Revoke {
            state_dir,
            public_key,
        } => revoke_peer(state_dir, public_key),
        Command::Help => {
            print_help();
            Ok(())
        }
    }
}

fn serve(bind: SocketAddr, state_dir: PathBuf, session_nonce: u64) -> Result<(), String> {
    let config = PairedLanConfig::selected_interface(bind, PresentationRole::GuidedOperator)
        .map_err(|error| format!("the selected LAN interface is not permitted: {error:?}"))?;
    let server_keys = Arc::new(
        load_or_create_server_identity(&state_dir)
            .map_err(|error| format!("could not load paired-LAN server identity: {error:?}"))?,
    );
    let registry = Arc::new(Mutex::new(load_peer_registry(&state_dir).map_err(
        |error| format!("could not load paired-LAN peer registry: {error:?}"),
    )?));

    let inner = FullMissionPresentationSession::new(OperationalRole::GuidedOperator)
        .map_err(|error| format!("could not compile guided GNSS-loss session: {error:?}"))?;
    let mut authority = PortableFullMissionAuthority::from_session(session_nonce, inner)
        .map_err(|error| format!("could not bind portable authority: {error:?}"))?;
    authority
        .prepare()
        .map_err(|error| format!("could not prepare guided GNSS-loss session: {error:?}"))?;
    let broker = Arc::new(
        SessionBrokerHandle::spawn(authority, WorkerConfig::default())
            .map_err(|error| format!("could not start authority worker: {error:?}"))?,
    );
    let service = PairedLanService::start(config, server_keys, registry, session_nonce, broker)
        .map_err(|error| format!("could not start paired-LAN service: {error:?}"))?;

    println!("KSA64 paired-LAN Mission Control is ready.");
    println!("Interface:   {}", service.local_addr());
    println!("Session:     {session_nonce:016x}");
    println!("Role:        Guided Operator (immutable for paired peers)");
    println!("State:       {}", state_dir.display());
    println!("Browser PWA: disabled (this is the separate native/Vita LAN lane)");
    println!("Type 'help' for local pairing, peer management, and shutdown commands.");

    let stdin = io::stdin();
    loop {
        print!("paired-lan> ");
        io::stdout()
            .flush()
            .map_err(|error| format!("could not write local prompt: {error}"))?;
        let mut line = String::new();
        let count = stdin
            .lock()
            .read_line(&mut line)
            .map_err(|error| format!("could not read local command: {error}"))?;
        if count == 0 {
            break;
        }
        match handle_local_command(&service, &state_dir, &line) {
            Ok(true) => break,
            Ok(false) => {}
            Err(error) => eprintln!("Command rejected: {error}"),
        }
    }
    drop(service);
    println!("KSA64 paired-LAN service stopped.");
    Ok(())
}

/// Returns true only for an explicit local shutdown request.
fn handle_local_command(
    service: &PairedLanService,
    state_dir: &std::path::Path,
    line: &str,
) -> Result<bool, String> {
    let parts: Vec<_> = line.split_ascii_whitespace().collect();
    match parts.as_slice() {
        [] => Ok(false),
        ["help"] => {
            println!("status | pending | confirm PAIRING_ID SIX_DIGIT_CODE | reject PAIRING_ID");
            println!("peers | revoke PUBLIC_KEY_HEX | quit");
            println!("Compare a pending code on the Vita and this local host before confirm.");
            Ok(false)
        }
        ["status"] => {
            println!("active connections: {}", service.active_connections());
            if let Some(error) = service.last_connection_error() {
                println!("last rejected connection: {error:?}");
            }
            print_pending(service);
            Ok(false)
        }
        ["pending"] => {
            print_pending(service);
            Ok(false)
        }
        ["confirm", pairing_id, code] => {
            let pairing_id = parse_u64(pairing_id, "pairing ID")?;
            let code = parse_comparison_code(code)?;
            service
                .confirm_pairing(pairing_id, code)
                .map_err(|error| format!("pairing confirmation failed: {error:?}"))?;
            persist_live_registry(service, state_dir)?;
            println!("Pairing confirmed locally; the client may now complete authentication.");
            Ok(false)
        }
        ["reject", pairing_id] => {
            service
                .reject_pairing(parse_u64(pairing_id, "pairing ID")?)
                .map_err(|error| format!("pairing rejection failed: {error:?}"))?;
            println!("Pairing rejected.");
            Ok(false)
        }
        ["peers"] => {
            print_live_peers(service)?;
            Ok(false)
        }
        ["revoke", public_key] => {
            let public_key = parse_public_key(public_key)?;
            service
                .revoke_peer(public_key)
                .map_err(|error| format!("could not revoke peer: {error:?}"))?;
            persist_live_registry(service, state_dir)?;
            println!(
                "Peer revoked. Any active client is rejected on its next bounded transport poll."
            );
            Ok(false)
        }
        ["quit"] | ["exit"] => Ok(true),
        _ => Err(String::from("unknown local command; type help")),
    }
}

fn persist_live_registry(
    service: &PairedLanService,
    state_dir: &std::path::Path,
) -> Result<(), String> {
    let bytes = service
        .export_peer_registry()
        .map_err(|error| format!("could not export peer registry: {error:?}"))?;
    save_peer_registry(state_dir, &bytes)
        .map_err(|error| format!("could not persist peer registry: {error:?}"))
}

fn print_pending(service: &PairedLanService) {
    if let Some(pending) = service.pending_pairing() {
        println!(
            "PENDING id={} role={:?} code={}",
            pending.pairing_id, pending.assigned_role, pending.comparison_code
        );
        println!(
            "Compare this code on the client, then type: confirm {} {}",
            pending.pairing_id, pending.comparison_code
        );
    } else {
        println!("No first-pairing request is waiting.");
    }
}

fn print_live_peers(service: &PairedLanService) -> Result<(), String> {
    let bytes = service
        .export_peer_registry()
        .map_err(|error| format!("could not export peer registry: {error:?}"))?;
    let registry = ksa64_session_broker::PeerRegistry::import_bounded(&bytes)
        .map_err(|error| format!("could not inspect peer registry: {error:?}"))?;
    print_registry(&registry);
    Ok(())
}

fn list_peers(state_dir: PathBuf) -> Result<(), String> {
    let registry = load_peer_registry(&state_dir)
        .map_err(|error| format!("could not load peer registry: {error:?}"))?;
    println!("State: {}", state_dir.display());
    print_registry(&registry);
    Ok(())
}

fn revoke_peer(state_dir: PathBuf, public_key: [u8; 32]) -> Result<(), String> {
    let mut registry = load_peer_registry(&state_dir)
        .map_err(|error| format!("could not load peer registry: {error:?}"))?;
    registry
        .revoke(&public_key)
        .map_err(|error| format!("could not revoke peer: {error:?}"))?;
    let bytes = registry
        .export_bounded()
        .map_err(|error| format!("could not encode peer registry: {error:?}"))?;
    save_peer_registry(&state_dir, &bytes)
        .map_err(|error| format!("could not persist peer registry: {error:?}"))?;
    println!("Peer revoked: {}", hex_key(&public_key));
    Ok(())
}

fn print_registry(registry: &ksa64_session_broker::PeerRegistry) {
    if registry.records().is_empty() {
        println!("No paired peers.");
        return;
    }
    for peer in registry.records() {
        println!(
            "{} role={:?} status={}",
            hex_key(&peer.public_key),
            peer.role,
            if peer.revoked { "revoked" } else { "active" }
        );
    }
}

fn parse_command(arguments: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let values: Vec<_> = arguments.into_iter().collect();
    if values.is_empty() || values == ["--help"] || values == ["-h"] {
        return Ok(Command::Help);
    }
    let mode = values[0].as_str();
    let mut state_dir = None;
    let mut bind = None;
    let mut session_nonce = None;
    let mut public_key = None;
    let mut index = 1;
    while index < values.len() {
        let key = &values[index];
        let value = values
            .get(index + 1)
            .ok_or_else(|| format!("{key} requires a value"))?;
        match key.as_str() {
            "--state-dir" => state_dir = Some(PathBuf::from(value)),
            "--bind" => {
                bind = Some(
                    value
                        .parse::<SocketAddr>()
                        .map_err(|_| String::from("--bind must be an explicit IP:PORT"))?,
                )
            }
            "--session-nonce" => session_nonce = Some(parse_nonce(value)?),
            "--public-key" => public_key = Some(parse_public_key(value)?),
            _ => return Err(format!("unknown argument: {key}")),
        }
        index += 2;
    }
    let state_dir = state_dir.ok_or_else(|| String::from("--state-dir is required"))?;
    match mode {
        "serve" => Ok(Command::Serve {
            bind: bind.ok_or_else(|| String::from("serve requires --bind IP:PORT"))?,
            state_dir,
            session_nonce: session_nonce
                .ok_or_else(|| String::from("serve requires --session-nonce HEX"))?,
        }),
        "list" => Ok(Command::List { state_dir }),
        "revoke" => Ok(Command::Revoke {
            state_dir,
            public_key: public_key
                .ok_or_else(|| String::from("revoke requires --public-key 64_HEX"))?,
        }),
        _ => Err(format!("unknown command: {mode}")),
    }
}

fn parse_nonce(text: &str) -> Result<u64, String> {
    let text = text.strip_prefix("0x").unwrap_or(text);
    let value = u64::from_str_radix(text, 16)
        .map_err(|_| String::from("--session-nonce must be a nonzero 64-bit hexadecimal value"))?;
    if value == 0 {
        return Err(String::from("--session-nonce must be nonzero"));
    }
    Ok(value)
}

fn parse_u64(text: &str, label: &str) -> Result<u64, String> {
    text.parse::<u64>()
        .map_err(|_| format!("{label} must be an unsigned integer"))
}

fn parse_comparison_code(text: &str) -> Result<ComparisonCode, String> {
    if text.len() != 6 || !text.bytes().all(|value| value.is_ascii_digit()) {
        return Err(String::from(
            "comparison code must contain exactly six digits",
        ));
    }
    ComparisonCode::from_value(
        text.parse()
            .map_err(|_| String::from("invalid comparison code"))?,
    )
    .ok_or_else(|| String::from("comparison code is out of range"))
}

fn parse_public_key(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 {
        return Err(String::from(
            "public key must contain exactly 64 hexadecimal characters",
        ));
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| String::from("public key must be hexadecimal"))?;
    }
    if output.iter().all(|value| *value == 0) {
        return Err(String::from("public key must not be all zero"));
    }
    Ok(output)
}

fn hex_key(value: &[u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn print_help() {
    println!("Usage:");
    println!(
        "  ksa64-paired-lan serve --bind PRIVATE_IP:PORT --state-dir DIRECTORY --session-nonce HEX"
    );
    println!("  ksa64-paired-lan list --state-dir DIRECTORY");
    println!("  ksa64-paired-lan revoke --state-dir DIRECTORY --public-key 64_HEX");
    println!();
    println!("This is the explicit native/Vita paired-LAN service. It never enables browser LAN,");
    println!("wildcard binds, discovery, UPnP, NAT traversal, or an Internet listener.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_parser_requires_explicit_lan_and_nonce() {
        assert!(parse_command([String::from("serve")]).is_err());
        assert!(parse_command([
            String::from("serve"),
            String::from("--state-dir"),
            String::from("state"),
            String::from("--bind"),
            String::from("0.0.0.0:27864"),
            String::from("--session-nonce"),
            String::from("4b534136"),
        ])
        .is_ok());
        assert!(parse_nonce("0").is_err());
        assert!(parse_comparison_code("12345").is_err());
        assert!(parse_public_key(&"00".repeat(32)).is_err());
        let command = parse_command([
            String::from("revoke"),
            String::from("--state-dir"),
            String::from("state"),
            String::from("--public-key"),
            "11".repeat(32),
        ])
        .unwrap();
        assert!(matches!(command, Command::Revoke { .. }));
    }
}
