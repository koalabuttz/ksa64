//! SDL2 PlayStation Vita presentation client.
//!
//! This program owns no mission authority. It starts with a bounded,
//! role-filtered offline replay; paired encrypted KPS1 transport can replace
//! that source without changing the view model or action boundary.

use core::ffi::{c_char, c_int, c_void};
use std::{
    format, fs,
    net::SocketAddr,
    string::{String, ToString},
};

use ksa64_presentation::PresentationRole;
use ksa64_vita_client::{
    paired_transport::{
        vita_secure_entropy, VitaLanClient, VitaLanConfig, VitaLanState, VitaPeerIdentity,
    },
    VitaConnection, VitaInput, VitaMissionControl, VitaPage, VitaViewModel, VITA_FRAME_RATE_TARGET,
    VITA_HEIGHT, VITA_WIDTH,
};

const SDL_INIT_VIDEO: u32 = 0x0000_0020;
const SDL_INIT_GAMECONTROLLER: u32 = 0x0000_2000;
const SDL_WINDOW_SHOWN: u32 = 0x0000_0004;
const SDL_QUIT: u32 = 0x100;
const SDL_CONTROLLERBUTTONDOWN: u32 = 0x653;
const SDL_BUTTON_A: u8 = 0;
const SDL_BUTTON_B: u8 = 1;
const SDL_BUTTON_Y: u8 = 3;
const SDL_BUTTON_BACK: u8 = 4;
const SDL_BUTTON_START: u8 = 6;
const SDL_BUTTON_DPAD_UP: u8 = 11;
const SDL_BUTTON_DPAD_DOWN: u8 = 12;
const SDL_BUTTON_DPAD_LEFT: u8 = 13;
const SDL_BUTTON_DPAD_RIGHT: u8 = 14;

#[repr(C)]
#[derive(Clone, Copy)]
struct SdlRect {
    x: c_int,
    y: c_int,
    w: c_int,
    h: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SdlControllerButtonEvent {
    kind: u32,
    timestamp: u32,
    which: i32,
    button: u8,
    state: u8,
    padding1: u8,
    padding2: u8,
}

#[repr(C)]
union SdlEvent {
    kind: u32,
    controller_button: SdlControllerButtonEvent,
    padding: [u8; 56],
}

#[cfg_attr(target_os = "vita", link(name = "SDL2", kind = "static"))]
unsafe extern "C" {
    fn SDL_Init(flags: u32) -> c_int;
    fn SDL_Quit();
    fn SDL_CreateWindow(
        title: *const c_char,
        x: c_int,
        y: c_int,
        w: c_int,
        h: c_int,
        flags: u32,
    ) -> *mut c_void;
    fn SDL_DestroyWindow(window: *mut c_void);
    fn SDL_CreateRenderer(window: *mut c_void, index: c_int, flags: u32) -> *mut c_void;
    fn SDL_DestroyRenderer(renderer: *mut c_void);
    fn SDL_SetRenderDrawColor(renderer: *mut c_void, r: u8, g: u8, b: u8, a: u8) -> c_int;
    fn SDL_RenderClear(renderer: *mut c_void) -> c_int;
    fn SDL_RenderPresent(renderer: *mut c_void);
    fn SDL_RenderFillRect(renderer: *mut c_void, rect: *const SdlRect) -> c_int;
    fn SDL_RenderDrawLine(
        renderer: *mut c_void,
        x1: c_int,
        y1: c_int,
        x2: c_int,
        y2: c_int,
    ) -> c_int;
    fn SDL_PollEvent(event: *mut SdlEvent) -> c_int;
    fn SDL_Delay(milliseconds: u32);
    fn SDL_GetTicks() -> u32;
    fn SDL_NumJoysticks() -> c_int;
    fn SDL_IsGameController(index: c_int) -> c_int;
    fn SDL_GameControllerOpen(index: c_int) -> *mut c_void;
    fn SDL_GameControllerClose(controller: *mut c_void);
}

#[derive(Clone, Copy)]
struct Color(u8, u8, u8);
const BG: Color = Color(5, 11, 20);
const PANEL: Color = Color(12, 25, 39);
const PANEL_ALT: Color = Color(18, 36, 53);
const CYAN: Color = Color(80, 221, 231);
const WHITE: Color = Color(232, 241, 244);
const MUTED: Color = Color(116, 148, 160);
const GREEN: Color = Color(83, 222, 139);
const AMBER: Color = Color(245, 189, 76);
const RED: Color = Color(245, 89, 94);

const VITA_LAN_SETTINGS_PATH: &str = "ux0:data/KSA64/vita-lan.conf";
const VITA_LAN_IDENTITY_PATH: &str = "ux0:data/KSA64/vita-peer.vpi";
const RECONNECT_INTERVAL_MILLIS: u32 = 2_000;
const PUBLICATION_POLL_MILLIS: u32 = 250;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LanStartupMode {
    Pair,
    Reconnect,
}

struct LanSettings {
    config: VitaLanConfig,
    mode: LanStartupMode,
}

/// SDL owns this small non-authoritative runtime. It is only created after an
/// explicit local configuration file has requested a private-LAN pairing or
/// reconnect. Absent or invalid configuration leaves the client in safe,
/// offline replay mode.
struct LiveRuntime {
    settings: LanSettings,
    transport: VitaLanClient,
    last_reconnect_tick: u32,
    last_publication_tick: u32,
}

impl LiveRuntime {
    fn pairing_code(&self) -> Option<String> {
        self.transport
            .comparison_code()
            .map(|code| format!("{code}"))
    }

    fn confirm_pairing(&mut self, client: &mut VitaMissionControl) -> Result<(), ()> {
        let code = self.transport.comparison_code().ok_or(())?;
        self.transport
            .confirm_pairing(code, client)
            .map_err(|_| ())?;
        persist_identity(self.transport.identity()).map_err(|_| ())?;
        Ok(())
    }

    fn tick(&mut self, client: &mut VitaMissionControl, now: u32) {
        if self.transport.tick(client).is_ok() {
            if self.transport.state() == VitaLanState::Active
                && now.wrapping_sub(self.last_publication_tick) >= PUBLICATION_POLL_MILLIS
            {
                self.last_publication_tick = now;
                let _ = self.transport.request_publication(client);
            }
            return;
        }
        if now.wrapping_sub(self.last_reconnect_tick) < RECONNECT_INTERVAL_MILLIS {
            return;
        }
        self.last_reconnect_tick = now;
        let identity = self.transport.identity();
        let entropy = match vita_secure_entropy() {
            Ok(value) => value,
            Err(_) => return,
        };
        if let Ok(transport) =
            VitaLanClient::begin_reconnect(self.settings.config, identity, entropy, client)
        {
            self.transport = transport;
            let _ = persist_identity(identity);
        }
    }
}

fn load_lan_settings() -> Option<LanSettings> {
    let bytes = fs::read(VITA_LAN_SETTINGS_PATH).ok()?;
    if bytes.len() > 1024 {
        return None;
    }
    let text = core::str::from_utf8(&bytes).ok()?;
    let mut mode = None;
    let mut host = None;
    let mut port = None;
    let mut nonce = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('=')?;
        if key.trim() != key || value.trim() != value || value.is_empty() {
            return None;
        }
        match key {
            "mode" if mode.is_none() => {
                mode = match value {
                    "pair" => Some(LanStartupMode::Pair),
                    "reconnect" => Some(LanStartupMode::Reconnect),
                    _ => return None,
                };
            }
            "host" if host.is_none() => host = Some(value),
            "port" if port.is_none() => port = value.parse::<u16>().ok(),
            "session_nonce" if nonce.is_none() => {
                let value = value.strip_prefix("0x").unwrap_or(value);
                nonce = u64::from_str_radix(value, 16).ok();
            }
            _ => return None,
        }
    }
    let address: SocketAddr = format!("{}:{}", host?, port?).parse().ok()?;
    let config = VitaLanConfig::paired(address, nonce?).ok()?;
    Some(LanSettings {
        config,
        mode: mode?,
    })
}

fn load_or_create_identity() -> Result<VitaPeerIdentity, ()> {
    match fs::read(VITA_LAN_IDENTITY_PATH) {
        Ok(bytes) => VitaPeerIdentity::decode(&bytes).map_err(|_| ()),
        Err(_) => {
            let identity = VitaPeerIdentity::generate(vita_secure_entropy().map_err(|_| ())?)
                .map_err(|_| ())?;
            persist_identity(identity)?;
            Ok(identity)
        }
    }
}

fn persist_identity(identity: VitaPeerIdentity) -> Result<(), ()> {
    let mut bytes = [0_u8; VitaPeerIdentity::ENCODED_LENGTH];
    let length = identity.encode(&mut bytes).map_err(|_| ())?;
    fs::create_dir_all("ux0:data/KSA64").map_err(|_| ())?;
    let temporary = "ux0:data/KSA64/vita-peer.tmp";
    fs::write(temporary, &bytes[..length]).map_err(|_| ())?;
    fs::rename(temporary, VITA_LAN_IDENTITY_PATH).map_err(|_| ())?;
    Ok(())
}

fn try_start_live(client: &mut VitaMissionControl) -> Option<LiveRuntime> {
    let settings = load_lan_settings()?;
    let identity = load_or_create_identity().ok()?;
    let entropy = vita_secure_entropy().ok()?;
    *client = VitaMissionControl::new(PresentationRole::GuidedOperator).ok()?;
    let transport = match settings.mode {
        LanStartupMode::Pair => {
            VitaLanClient::begin_pairing(settings.config, identity, entropy).ok()?
        }
        LanStartupMode::Reconnect => {
            VitaLanClient::begin_reconnect(settings.config, identity, entropy, client).ok()?
        }
    };
    Some(LiveRuntime {
        settings,
        transport,
        last_reconnect_tick: 0,
        last_publication_tick: 0,
    })
}

fn main() {
    let mut client = match VitaMissionControl::offline_replay(PresentationRole::GuidedOperator) {
        Ok(client) => client,
        Err(_) => return,
    };
    client.load_offline_fixture();
    // A LAN config is an explicit opt-in. Network initialization or pairing
    // failure deliberately leaves the safe offline replay available.
    let _network = if load_lan_settings().is_some() {
        VitaNetwork::initialize().ok()
    } else {
        None
    };
    let mut live = if _network.is_some() {
        try_start_live(&mut client)
    } else {
        None
    };

    unsafe {
        if SDL_Init(SDL_INIT_VIDEO | SDL_INIT_GAMECONTROLLER) < 0 {
            return;
        }
        let title = b"KSA64 Mission Control\0";
        let window = SDL_CreateWindow(
            title.as_ptr().cast(),
            0,
            0,
            VITA_WIDTH.into(),
            VITA_HEIGHT.into(),
            SDL_WINDOW_SHOWN,
        );
        if window.is_null() {
            SDL_Quit();
            return;
        }
        let renderer = SDL_CreateRenderer(window, -1, 0);
        if renderer.is_null() {
            SDL_DestroyWindow(window);
            SDL_Quit();
            return;
        }
        let controller = open_first_controller();
        let mut running = true;
        let mut timeline_offset = 0_usize;
        while running {
            let mut event = SdlEvent { padding: [0; 56] };
            while SDL_PollEvent(&mut event) != 0 {
                let kind = event.kind;
                if kind == SDL_QUIT {
                    running = false;
                    continue;
                }
                if kind != SDL_CONTROLLERBUTTONDOWN {
                    continue;
                }
                let button = event.controller_button.button;
                match button {
                    SDL_BUTTON_START => running = false,
                    SDL_BUTTON_DPAD_LEFT => {
                        let _ = client.handle_input(VitaInput::Left);
                        timeline_offset = 0;
                    }
                    SDL_BUTTON_DPAD_RIGHT => {
                        let _ = client.handle_input(VitaInput::Right);
                        timeline_offset = 0;
                    }
                    SDL_BUTTON_DPAD_UP => timeline_offset = timeline_offset.saturating_add(1),
                    SDL_BUTTON_DPAD_DOWN => timeline_offset = timeline_offset.saturating_sub(1),
                    SDL_BUTTON_A => {
                        if let Some(runtime) = live.as_mut() {
                            if runtime.transport.state() == VitaLanState::PairingCodePending {
                                let _ = runtime.confirm_pairing(&mut client);
                            } else if let Ok(Some(intent)) = client.handle_input(VitaInput::Cross) {
                                let _ = runtime.transport.submit_intent(&mut client, intent);
                            }
                        } else {
                            let _ = client.handle_input(VitaInput::Cross);
                        }
                    }
                    SDL_BUTTON_B => {
                        if let Ok(Some(intent)) = client.handle_input(VitaInput::Circle) {
                            if let Some(runtime) = live.as_mut() {
                                let _ = runtime.transport.submit_intent(&mut client, intent);
                            }
                        }
                    }
                    SDL_BUTTON_Y => {
                        if let Some(runtime) = live.as_mut() {
                            let _ = runtime.transport.request_resync(&mut client);
                        } else if client.connection() == VitaConnection::ResyncRequired {
                            client.reset_for_resync();
                        }
                    }
                    SDL_BUTTON_BACK => timeline_offset = 0,
                    _ => {}
                }
            }
            if let Some(runtime) = live.as_mut() {
                runtime.tick(&mut client, SDL_GetTicks());
            }
            render(renderer, &client, live.as_ref(), timeline_offset);
            SDL_RenderPresent(renderer);
            SDL_Delay(1_000 / u32::from(VITA_FRAME_RATE_TARGET));
        }
        if !controller.is_null() {
            SDL_GameControllerClose(controller);
        }
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
        SDL_Quit();
    }
}

#[cfg(target_os = "vita")]
#[repr(C)]
struct SceNetInitParam {
    memory: *mut c_void,
    size: i32,
    flags: i32,
}
#[cfg(target_os = "vita")]
#[link(name = "SceNet_stub")]
unsafe extern "C" {
    fn sceNetInit(param: *mut SceNetInitParam) -> i32;
    fn sceNetTerm() -> i32;
}

/// Keeps the VitaSDK network work buffer alive for the whole SDL session.
/// There is no Internet listener: the client only makes an explicit connection
/// to the configured private-LAN broker.
struct VitaNetwork {
    #[cfg(target_os = "vita")]
    memory: Vec<u8>,
}
impl VitaNetwork {
    #[cfg(target_os = "vita")]
    fn initialize() -> Result<Self, ()> {
        let mut memory = vec![0_u8; 1024 * 1024];
        let mut param = SceNetInitParam {
            memory: memory.as_mut_ptr().cast(),
            size: memory.len() as i32,
            flags: 0,
        };
        if unsafe { sceNetInit(&mut param) } < 0 {
            return Err(());
        }
        Ok(Self { memory })
    }
    #[cfg(not(target_os = "vita"))]
    fn initialize() -> Result<Self, ()> {
        Err(())
    }
}
#[cfg(target_os = "vita")]
impl Drop for VitaNetwork {
    fn drop(&mut self) {
        let _ = self.memory.len();
        let _ = unsafe { sceNetTerm() };
    }
}

unsafe fn open_first_controller() -> *mut c_void {
    for index in 0..SDL_NumJoysticks() {
        if SDL_IsGameController(index) != 0 {
            return SDL_GameControllerOpen(index);
        }
    }
    core::ptr::null_mut()
}

unsafe fn render(
    renderer: *mut c_void,
    client: &VitaMissionControl,
    live: Option<&LiveRuntime>,
    timeline_offset: usize,
) {
    color(renderer, BG);
    SDL_RenderClear(renderer);
    fill(renderer, 0, 0, 960, 54, PANEL_ALT);
    fill(renderer, 0, 510, 960, 34, PANEL_ALT);
    text(renderer, 22, 16, 2, CYAN, "KSA64 / MISSION CONTROL");
    text(renderer, 632, 12, 1, MUTED, "ROLE: GUIDED OPERATOR");
    text(
        renderer,
        632,
        29,
        1,
        connection_color(client.connection()),
        &format!("LINK: {:?}", client.connection()),
    );
    if let Some(runtime) = live {
        let detail = runtime
            .pairing_code()
            .map(|code| format!("PAIR CODE {code} / CONFIRM CROSS"))
            .unwrap_or_else(|| format!("LAN {}", runtime.transport.state()));
        text(
            renderer,
            320,
            44,
            1,
            if runtime.transport.state() == VitaLanState::PairingCodePending {
                AMBER
            } else {
                GREEN
            },
            &detail,
        );
    }

    let pages = [
        VitaPage::Status,
        VitaPage::Navigation,
        VitaPage::Procedure,
        VitaPage::Trajectory,
        VitaPage::Timeline,
        VitaPage::Evidence,
    ];
    for (index, page) in pages.iter().enumerate() {
        let x = 12 + index as i32 * 157;
        let active = *page == client.page();
        if active {
            fill(renderer, x, 61, 147, 25, PANEL_ALT);
        }
        text(
            renderer,
            x + 8,
            69,
            1,
            if active { CYAN } else { MUTED },
            page_name(*page),
        );
    }

    let view = client.view_model();
    match client.page() {
        VitaPage::Status => render_status(renderer, &view),
        VitaPage::Navigation => render_navigation(renderer, &view),
        VitaPage::Procedure => render_procedure(renderer, &view),
        VitaPage::Trajectory => render_trajectory(renderer, client),
        VitaPage::Timeline => render_timeline(renderer, client, timeline_offset),
        VitaPage::Evidence => render_evidence(renderer, &view, client),
    }
    text(
        renderer,
        18,
        522,
        1,
        MUTED,
        "LEFT/RIGHT PAGE   UP/DOWN SCROLL   CROSS ACTION   CIRCLE CANCEL   START EXIT",
    );
}

unsafe fn render_status(renderer: *mut c_void, view: &VitaViewModel) {
    panel(renderer, 18, 98, 450, 190);
    panel(renderer, 486, 98, 456, 190);
    panel(renderer, 18, 306, 924, 186);
    heading(renderer, 34, 116, "FLIGHT DIRECTOR");
    if let Some(snapshot) = &view.snapshot {
        value(
            renderer,
            34,
            148,
            "MISSION TIME",
            &format!("{:.1} S", q16(snapshot.mission_time_q16)),
        );
        value(
            renderer,
            34,
            174,
            "RELEASE",
            &snapshot.release_epoch.to_string(),
        );
        value(
            renderer,
            34,
            200,
            "FRAME",
            &snapshot.frame_identity.to_string(),
        );
        value(
            renderer,
            34,
            226,
            "GNSS",
            if snapshot.gnss_state == 3 {
                "INVALID / INERTIAL"
            } else {
                "VALID"
            },
        );
        value(
            renderer,
            34,
            252,
            "SAFE",
            if snapshot.safe { "YES" } else { "NO" },
        );
    }
    heading(renderer, 502, 116, "MISSION DISPOSITION");
    if let Some(disposition) = view.disposition {
        text(
            renderer,
            502,
            150,
            2,
            GREEN,
            &format!("{:?}", disposition.overall),
        );
        value(
            renderer,
            502,
            190,
            "OBJECTIVE",
            &disposition.axes.objective.to_string(),
        );
        value(
            renderer,
            502,
            216,
            "VEHICLE / AVIONICS",
            &format!(
                "{} / {}",
                disposition.axes.vehicle, disposition.axes.avionics
            ),
        );
        value(
            renderer,
            502,
            242,
            "PROCEDURE / OPERATOR",
            &format!(
                "{} / {}",
                disposition.axes.procedure, disposition.axes.operator
            ),
        );
    }
    heading(renderer, 34, 324, "STATUS");
    wrapped_text(renderer, 34, 356, 72, 3, WHITE, &view.status_line);
    text(
        renderer,
        34,
        433,
        1,
        AMBER,
        "PLAN DEVIATION DOES NOT AUTOMATICALLY MEAN MISSION FAILURE",
    );
    text(
        renderer,
        34,
        456,
        1,
        MUTED,
        "THIS CLIENT IS PRESENTATION-ONLY; REMOTE AUTHORITY CONTINUES THROUGH DISCONNECTS",
    );
}

unsafe fn render_navigation(renderer: *mut c_void, view: &VitaViewModel) {
    panel(renderer, 18, 98, 452, 394);
    panel(renderer, 488, 98, 454, 394);
    heading(renderer, 34, 116, "ONBOARD ESTIMATE");
    heading(renderer, 504, 116, "GROUND ESTIMATE / RESIDUAL");
    if let Some(snapshot) = &view.snapshot {
        nav_block(
            renderer,
            34,
            154,
            snapshot.onboard.position_q12_km,
            snapshot.onboard.velocity_q24_km_s,
            snapshot.onboard.checksum,
        );
        nav_block(
            renderer,
            504,
            154,
            snapshot.ground.position_q12_km,
            snapshot.ground.velocity_q24_km_s,
            snapshot.ground.checksum,
        );
        let residual = [
            snapshot.ground.position_q12_km[0] - snapshot.onboard.position_q12_km[0],
            snapshot.ground.position_q12_km[1] - snapshot.onboard.position_q12_km[1],
            snapshot.ground.position_q12_km[2] - snapshot.onboard.position_q12_km[2],
        ];
        heading(renderer, 504, 320, "POSITION RESIDUAL");
        text(
            renderer,
            504,
            352,
            1,
            AMBER,
            &format!("X {:>8.3} KM", q12(residual[0])),
        );
        text(
            renderer,
            504,
            376,
            1,
            AMBER,
            &format!("Y {:>8.3} KM", q12(residual[1])),
        );
        text(
            renderer,
            504,
            400,
            1,
            AMBER,
            &format!("Z {:>8.3} KM", q12(residual[2])),
        );
        text(
            renderer,
            504,
            447,
            1,
            MUTED,
            "TRUTH DATA IS NOT PRESENT IN THIS ROLE",
        );
    }
}

unsafe fn nav_block(
    renderer: *mut c_void,
    x: i32,
    y: i32,
    position: [i32; 3],
    velocity: [i32; 3],
    checksum: u32,
) {
    text(renderer, x, y, 1, MUTED, "POSITION KM");
    for axis in 0..3 {
        text(
            renderer,
            x,
            y + 24 + axis as i32 * 24,
            1,
            WHITE,
            &format!("{} {:>10.3}", ['X', 'Y', 'Z'][axis], q12(position[axis])),
        );
    }
    text(renderer, x, y + 112, 1, MUTED, "VELOCITY KM/S");
    for axis in 0..3 {
        text(
            renderer,
            x,
            y + 136 + axis as i32 * 24,
            1,
            CYAN,
            &format!("{} {:>10.5}", ['X', 'Y', 'Z'][axis], q24(velocity[axis])),
        );
    }
    text(
        renderer,
        x,
        y + 224,
        1,
        MUTED,
        &format!("CHECKSUM {:08X}", checksum),
    );
}

unsafe fn render_procedure(renderer: *mut c_void, view: &VitaViewModel) {
    panel(renderer, 18, 98, 924, 394);
    heading(renderer, 34, 116, "ACTIVE PROCEDURE");
    if let Some(procedure) = &view.procedure {
        text(renderer, 34, 150, 2, CYAN, &procedure.title);
        value(
            renderer,
            34,
            192,
            "STEP",
            &format!("{} / {}", procedure.active_step, procedure.step_count),
        );
        value(
            renderer,
            34,
            218,
            "STATE",
            &format!("{:?}", procedure.state),
        );
        wrapped_text(renderer, 34, 258, 72, 4, WHITE, &procedure.instruction);
        heading(renderer, 34, 354, "PUBLIC GUARDS");
        for (index, predicate) in procedure.predicates.iter().take(4).enumerate() {
            text(
                renderer,
                34,
                382 + index as i32 * 23,
                1,
                if predicate.satisfied { GREEN } else { AMBER },
                &format!(
                    "{}  GUARD {:08X}",
                    if predicate.satisfied { "PASS" } else { "WAIT" },
                    predicate.identity
                ),
            );
        }
    } else {
        text(renderer, 34, 160, 1, MUTED, "NO ACTIVE PROCEDURE");
    }
}

unsafe fn render_trajectory(renderer: *mut c_void, client: &VitaMissionControl) {
    panel(renderer, 18, 98, 924, 394);
    heading(renderer, 34, 116, "ALTITUDE / DOWNRANGE - ONBOARD ESTIMATE");
    let left = 56;
    let top = 150;
    let width = 850;
    let height = 292;
    color(renderer, MUTED);
    SDL_RenderDrawLine(renderer, left, top + height, left + width, top + height);
    SDL_RenderDrawLine(renderer, left, top, left, top + height);
    let samples = client.samples();
    let max_x = samples
        .iter()
        .map(|s| s.downrange_q12_km)
        .max()
        .unwrap_or(1)
        .max(1) as i64;
    let max_y = samples
        .iter()
        .map(|s| s.altitude_q12_km)
        .max()
        .unwrap_or(1)
        .max(1) as i64;
    color(renderer, CYAN);
    for pair in samples.windows(2) {
        let x1 = left + (i64::from(pair[0].downrange_q12_km) * i64::from(width) / max_x) as i32;
        let y1 =
            top + height - (i64::from(pair[0].altitude_q12_km) * i64::from(height) / max_y) as i32;
        let x2 = left + (i64::from(pair[1].downrange_q12_km) * i64::from(width) / max_x) as i32;
        let y2 =
            top + height - (i64::from(pair[1].altitude_q12_km) * i64::from(height) / max_y) as i32;
        SDL_RenderDrawLine(renderer, x1, y1, x2, y2);
    }
    text(
        renderer,
        56,
        458,
        1,
        MUTED,
        &format!(
            "0 KM                                      {:.0} KM DOWNRANGE",
            q12(max_x as i32)
        ),
    );
    text(
        renderer,
        67,
        164,
        1,
        MUTED,
        &format!("APOGEE {:.1} KM", q12(max_y as i32)),
    );
}

unsafe fn render_timeline(renderer: *mut c_void, client: &VitaMissionControl, offset: usize) {
    panel(renderer, 18, 98, 924, 394);
    heading(renderer, 34, 116, "EVENT TIMELINE");
    let events = client.timeline();
    let end = events.len().saturating_sub(offset.min(events.len()));
    let start = end.saturating_sub(12);
    for (row, event) in events[start..end].iter().enumerate() {
        let y = 152 + row as i32 * 27;
        text(
            renderer,
            34,
            y,
            1,
            severity_color(event.severity),
            &format!(
                "T+{:>6.2}  {:<8?}  {}",
                event.release_epoch as f32 / 32.0,
                event.severity,
                event.label
            ),
        );
    }
    if client.timeline_overflowed() {
        text(
            renderer,
            34,
            463,
            1,
            RED,
            "RETENTION GAP / RESYNCHRONIZATION REQUIRED",
        );
    }
}

unsafe fn render_evidence(
    renderer: *mut c_void,
    view: &VitaViewModel,
    client: &VitaMissionControl,
) {
    panel(renderer, 18, 98, 560, 394);
    panel(renderer, 596, 98, 346, 394);
    heading(renderer, 34, 116, "SEALED EVIDENCE");
    if let Some(evidence) = view.evidence {
        value(
            renderer,
            34,
            154,
            "IDENTITY",
            &format!("{:08X}", evidence.evidence_identity),
        );
        value(
            renderer,
            34,
            180,
            "CRC32",
            &format!("{:08X}", evidence.evidence_crc32),
        );
        value(
            renderer,
            34,
            206,
            "BYTES",
            &evidence.total_length.to_string(),
        );
        value(
            renderer,
            34,
            232,
            "CHUNKS",
            &evidence.chunk_count.to_string(),
        );
        value(
            renderer,
            34,
            258,
            "COMPLETE",
            if evidence.complete {
                "YES"
            } else {
                "NO / INCOMPLETE"
            },
        );
    }
    heading(renderer, 34, 310, "CHECKSUM CHAINS");
    if let Some(snapshot) = &view.snapshot {
        text(
            renderer,
            34,
            344,
            1,
            WHITE,
            &format!("FLIGHT    {:08X}", snapshot.flight_checksum),
        );
        text(
            renderer,
            34,
            368,
            1,
            WHITE,
            &format!("COMMAND   {:08X}", snapshot.command_checksum),
        );
        text(
            renderer,
            34,
            392,
            1,
            WHITE,
            &format!("PROCEDURE {:08X}", snapshot.procedure_chain),
        );
        text(
            renderer,
            34,
            416,
            1,
            WHITE,
            &format!("ACTION    {:08X}", snapshot.action_chain),
        );
    }
    heading(renderer, 612, 116, "CLIENT BOUNDS");
    let budget = client.memory_budget();
    value(
        renderer,
        612,
        154,
        "TARGET FPS",
        &VITA_FRAME_RATE_TARGET.to_string(),
    );
    value(
        renderer,
        612,
        180,
        "STATE BUDGET",
        &format!("{} MIB", budget.total_bytes / 1024 / 1024),
    );
    value(renderer, 612, 206, "LIMIT", "64 MIB");
    value(
        renderer,
        612,
        232,
        "RECEIPTS",
        &client.receipts().len().to_string(),
    );
    value(
        renderer,
        612,
        258,
        "ACTION",
        &format!("{:?}", view.action_state),
    );
    wrapped_text(
        renderer,
        612,
        318,
        26,
        5,
        MUTED,
        "VPK BUILD PROVES PACKAGING ONLY. VITA3K AND PHYSICAL DEVICE ACCEPTANCE REMAIN PENDING.",
    );
}

unsafe fn panel(renderer: *mut c_void, x: i32, y: i32, w: i32, h: i32) {
    fill(renderer, x, y, w, h, PANEL);
}
unsafe fn heading(renderer: *mut c_void, x: i32, y: i32, label: &str) {
    text(renderer, x, y, 1, CYAN, label);
}
unsafe fn value(renderer: *mut c_void, x: i32, y: i32, label: &str, value: &str) {
    text(renderer, x, y, 1, MUTED, label);
    text(renderer, x + 190, y, 1, WHITE, value);
}
unsafe fn fill(renderer: *mut c_void, x: i32, y: i32, w: i32, h: i32, c: Color) {
    color(renderer, c);
    let rect = SdlRect { x, y, w, h };
    SDL_RenderFillRect(renderer, &rect);
}
unsafe fn color(renderer: *mut c_void, c: Color) {
    SDL_SetRenderDrawColor(renderer, c.0, c.1, c.2, 255);
}

unsafe fn wrapped_text(
    renderer: *mut c_void,
    x: i32,
    y: i32,
    columns: usize,
    max_lines: usize,
    c: Color,
    input: &str,
) {
    let upper = input.to_ascii_uppercase();
    let words: Vec<&str> = upper.split_whitespace().collect();
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in words {
        let needed = if line.is_empty() {
            word.len()
        } else {
            line.len() + 1 + word.len()
        };
        if needed > columns && !line.is_empty() {
            lines.push(line);
            line = String::new();
            if lines.len() == max_lines {
                break;
            }
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() && lines.len() < max_lines {
        lines.push(line);
    }
    for (index, line) in lines.iter().enumerate() {
        text(renderer, x, y + index as i32 * 20, 1, c, line);
    }
}

unsafe fn text(renderer: *mut c_void, x: i32, y: i32, scale: i32, c: Color, input: &str) {
    color(renderer, c);
    let mut cursor = x;
    for character in input.to_ascii_uppercase().chars() {
        if cursor > 952 {
            break;
        }
        let rows = glyph(character);
        for (row, bits) in rows.iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) != 0 {
                    let rect = SdlRect {
                        x: cursor + column * scale,
                        y: y + row as i32 * scale,
                        w: scale,
                        h: scale,
                    };
                    SDL_RenderFillRect(renderer, &rect);
                }
            }
        }
        cursor += 6 * scale;
    }
}

fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 2, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        ':' => [0, 4, 4, 0, 4, 4, 0],
        '.' => [0, 0, 0, 0, 0, 6, 6],
        ',' => [0, 0, 0, 0, 6, 6, 4],
        '(' => [2, 4, 8, 8, 8, 4, 2],
        ')' => [8, 4, 2, 2, 2, 4, 8],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '_' => [0, 0, 0, 0, 0, 0, 31],
        '=' => [0, 0, 31, 0, 31, 0, 0],
        '?' => [14, 17, 1, 2, 4, 0, 4],
        _ => [0; 7],
    }
}
fn q12(raw: i32) -> f32 {
    raw as f32 / 4096.0
}
fn q16(raw: u32) -> f32 {
    raw as f32 / 65536.0
}
fn q24(raw: i32) -> f32 {
    raw as f32 / 16_777_216.0
}
fn page_name(page: VitaPage) -> &'static str {
    match page {
        VitaPage::Status => "STATUS",
        VitaPage::Navigation => "NAVIGATION",
        VitaPage::Procedure => "PROCEDURE",
        VitaPage::Trajectory => "TRAJECTORY",
        VitaPage::Timeline => "TIMELINE",
        VitaPage::Evidence => "EVIDENCE",
    }
}
fn connection_color(connection: VitaConnection) -> Color {
    match connection {
        VitaConnection::Current | VitaConnection::OfflineReplay => GREEN,
        VitaConnection::Connecting | VitaConnection::Stale => AMBER,
        VitaConnection::ResyncRequired | VitaConnection::Closed => RED,
    }
}
fn severity_color(severity: ksa64_presentation::TimelineSeverity) -> Color {
    match severity {
        ksa64_presentation::TimelineSeverity::Information => WHITE,
        ksa64_presentation::TimelineSeverity::Caution => AMBER,
        ksa64_presentation::TimelineSeverity::Warning
        | ksa64_presentation::TimelineSeverity::Critical => RED,
    }
}
