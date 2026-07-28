use std::{
    env, fs,
    io::{self, BufRead},
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::Duration,
};

use ksa64_interface::phase11::OperationalRole;
use ksa64_presentation::PresentationRole;
use ksa64_session::presentation_adapter::FullMissionPresentationSession;
use ksa64_session_broker::{
    BrowserLaunchToken, BrowserServiceConfig, EmbeddedStaticAssets, LoopbackWebService,
    PortableFullMissionAuthority, SessionBrokerHandle, StaticAssetProvider, WorkerConfig,
    BROWSER_SUBPROTOCOL_PREFIX, PRESENTATION_WEBSOCKET_PATH,
};

const DEFAULT_PORT: u16 = 8765;
const DEFAULT_WEB_ROOT: &str = "web/dist";

#[derive(Debug)]
struct LauncherConfig {
    port: u16,
    web_root: PathBuf,
    run_for: Option<Duration>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("KSA64 broker error: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let config = parse_args(env::args().skip(1))?;
    let origin = format!("http://127.0.0.1:{}", config.port);
    let websocket = format!(
        "ws://127.0.0.1:{}{}",
        config.port, PRESENTATION_WEBSOCKET_PATH
    );
    let token = BrowserLaunchToken::generate()
        .map_err(|error| format!("could not generate browser launch token: {error:?}"))?;
    let subprotocol = token.subprotocol();
    let bare_token = subprotocol
        .strip_prefix(BROWSER_SUBPROTOCOL_PREFIX)
        .ok_or_else(|| String::from("generated subprotocol has the wrong identity"))?;
    let assets = load_pwa_assets(&config.web_root, &websocket, bare_token, &origin)?;

    let session_nonce = random_nonzero_u64()?;
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
    let browser_config = BrowserServiceConfig::loopback(
        config.port,
        [origin.clone()],
        PresentationRole::GuidedOperator,
    )
    .map_err(|error| format!("invalid loopback service configuration: {error:?}"))?;
    let service = LoopbackWebService::start(
        browser_config,
        token,
        session_nonce,
        broker.clone(),
        Arc::new(assets),
    )
    .map_err(|error| format!("could not start loopback service: {error:?}"))?;

    println!("KSA64 guided GNSS-loss Mission Control is ready.");
    println!("URL:         {origin}/");
    println!("Origin:      {origin}");
    println!("WebSocket:   {websocket}");
    println!("Credential:  injected into uncached runtime configuration (not logged)");
    println!("Session:     {session_nonce:016x}");
    println!("LAN:         disabled (paired LAN is never started implicitly)");

    if let Some(duration) = config.run_for {
        println!(
            "The bounded launcher will stop after {} second(s).",
            duration.as_secs()
        );
        thread::sleep(duration);
    } else {
        println!("Press Enter to stop the broker cleanly.");
        let mut line = String::new();
        io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(|error| format!("could not read shutdown request: {error}"))?;
    }

    service.shutdown();
    drop(broker);
    println!("KSA64 broker stopped.");
    Ok(())
}

fn parse_args(arguments: impl IntoIterator<Item = String>) -> Result<LauncherConfig, String> {
    let mut port = DEFAULT_PORT;
    let mut web_root = PathBuf::from(DEFAULT_WEB_ROOT);
    let mut run_for = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--port" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| String::from("--port requires a value"))?;
                port = value
                    .parse::<u16>()
                    .map_err(|_| String::from("--port must be an integer from 1 through 65535"))?;
                if port == 0 {
                    return Err(String::from(
                        "--port 0 is forbidden because Origin is exact",
                    ));
                }
            }
            "--web-root" => {
                web_root = PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| String::from("--web-root requires a directory"))?,
                );
            }
            "--run-for-seconds" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| String::from("--run-for-seconds requires a value"))?;
                let seconds = value
                    .parse::<u64>()
                    .map_err(|_| String::from("--run-for-seconds must be a positive integer"))?;
                if seconds == 0 || seconds > 86_400 {
                    return Err(String::from(
                        "--run-for-seconds must be between 1 and 86400",
                    ));
                }
                run_for = Some(Duration::from_secs(seconds));
            }
            "--help" | "-h" => {
                println!(
                    "Usage: ksa64-session-broker [--port PORT] [--web-root DIRECTORY] \
                     [--run-for-seconds N]\n\nStarts the accepted guided GNSS-loss authority and the \
                     loopback-only PWA broker. Paired LAN is not started by this command."
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok(LauncherConfig {
        port,
        web_root,
        run_for,
    })
}

fn load_pwa_assets(
    root: &Path,
    websocket: &str,
    browser_token: &str,
    allowed_origin: &str,
) -> Result<EmbeddedStaticAssets, String> {
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        format!(
            "PWA build directory {} is unavailable: {error}",
            root.display()
        )
    })?;
    if !canonical_root.is_dir() {
        return Err(format!(
            "PWA build path is not a directory: {}",
            root.display()
        ));
    }
    let mut files = Vec::new();
    collect_files(&canonical_root, &canonical_root, &mut files)?;
    files.sort();
    let mut assets = EmbeddedStaticAssets::default();
    for file in files {
        let relative = file
            .strip_prefix(&canonical_root)
            .map_err(|_| String::from("PWA file escaped its build directory"))?;
        let relative_text = relative
            .to_str()
            .ok_or_else(|| String::from("PWA paths must be valid UTF-8"))?
            .replace('\\', "/");
        if relative_text
            .split('/')
            .any(|component| component.starts_with('.'))
        {
            continue;
        }
        let route = format!("/{relative_text}");
        let mut body = fs::read(&file)
            .map_err(|error| format!("could not read {}: {error}", file.display()))?;
        if route == "/runtime-config.js" {
            body = runtime_config_script(websocket, browser_token, allowed_origin);
        }
        let (content_type, cache_control) = static_headers(&route)?;
        assets
            .insert(route, content_type, cache_control, body)
            .map_err(|error| format!("PWA asset set is invalid or oversized: {error:?}"))?;
    }
    if assets.lookup("/index.html").is_none() {
        return Err(String::from("PWA build has no index.html"));
    }
    Ok(assets)
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("could not enumerate {}: {error}", directory.display()))?
    {
        let entry =
            entry.map_err(|error| format!("could not read PWA directory entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("could not inspect {}: {error}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "PWA build may not contain symbolic links: {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            collect_files(root, &entry.path(), files)?;
        } else if file_type.is_file() {
            let canonical = fs::canonicalize(entry.path()).map_err(|error| {
                format!("could not resolve {}: {error}", entry.path().display())
            })?;
            if !canonical.starts_with(root) {
                return Err(String::from("PWA file escaped its build directory"));
            }
            files.push(canonical);
        }
    }
    Ok(())
}

fn runtime_config_script(websocket: &str, browser_token: &str, allowed_origin: &str) -> Vec<u8> {
    format!(
        "window.__KSA64_PRESENTATION__={{mode:\"remote-websocket\",endpoint:\"{websocket}\",browserToken:\"{browser_token}\",allowedOrigin:\"{allowed_origin}\"}};"
    )
    .into_bytes()
}

fn static_headers(route: &str) -> Result<(&'static str, &'static str), String> {
    let content_type = if route.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if route.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if route.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if route.ends_with(".wasm") {
        "application/wasm"
    } else if route.ends_with(".svg") {
        "image/svg+xml"
    } else if route.ends_with(".webmanifest") || route.ends_with(".json") {
        "application/json"
    } else if route.ends_with(".sha256") {
        "text/plain; charset=utf-8"
    } else {
        return Err(format!("unsupported PWA asset type: {route}"));
    };
    let cache_control = if route == "/runtime-config.js" {
        "no-store"
    } else if route == "/index.html" || route == "/sw.js" || route.ends_with(".webmanifest") {
        "no-cache"
    } else if route.starts_with("/assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    Ok((content_type, cache_control))
}

fn random_nonzero_u64() -> Result<u64, String> {
    for _ in 0..8 {
        let mut bytes = [0_u8; 8];
        getrandom::fill(&mut bytes)
            .map_err(|error| format!("could not generate session nonce: {error}"))?;
        let value = u64::from_le_bytes(bytes);
        if value != 0 {
            return Ok(value);
        }
    }
    Err(String::from("could not generate a nonzero session nonce"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_bounded_and_runtime_config_is_injected() {
        let config = parse_args([
            String::from("--port"),
            String::from("9123"),
            String::from("--web-root"),
            String::from("site"),
            String::from("--run-for-seconds"),
            String::from("2"),
        ])
        .unwrap();
        assert_eq!(config.port, 9123);
        assert_eq!(config.web_root, PathBuf::from("site"));
        assert_eq!(config.run_for, Some(Duration::from_secs(2)));
        assert!(parse_args([String::from("--port"), String::from("0")]).is_err());
        assert!(parse_args([String::from("--run-for-seconds"), String::from("0")]).is_err());

        let text = String::from_utf8(runtime_config_script(
            "ws://127.0.0.1:9123/api/presentation/v1",
            &"ab".repeat(32),
            "http://127.0.0.1:9123",
        ))
        .unwrap();
        assert!(text.contains("remote-websocket"));
        assert!(!text.contains("ksa64.presentation.v1.token."));
        assert!(text.contains(&"ab".repeat(32)));
        assert_eq!(static_headers("/runtime-config.js").unwrap().1, "no-store");
    }
}
