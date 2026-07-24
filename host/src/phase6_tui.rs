//! Ratatui presentation layer for live and recorded Phase 6 Mission Control.
use crate::phase6_audio::AudioEngine;
use crate::phase6_runner::{
    run_native_host_mission_controlled, run_world_with_flight_controlled, MissionControlSink,
    MissionControlUpdate, PaceController, RunnerError, RunnerEvidence, RunnerOptions,
};
use crate::phase6_session::{
    default_session_path, RecordingSink, Session, SessionError, SessionRecorder,
};
use crate::phase6_trajectory::{
    environment_from_observed_raw, flight_path_angle, format_orbit_kind, great_circle_downrange,
    latitude_longitude, orbit_from_state, project_to_plane, propagate_elliptic,
    residual_in_plan_frame, sample_orbit, split_antimeridian, time_to_apsis, PlanReference,
    Residual, Vec3, EARTH_RADIUS_KM, FAST_EPOCH_HZ, LAUNCH_LATITUDE_DEG, LAUNCH_LONGITUDE_DEG,
    PLAN_STREAM_CRC32, TARGET_ALTITUDE_KM, TARGET_APSIS_MAX_KM, TARGET_APSIS_MIN_KM,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols::{border, Marker};
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Circle, Line as CanvasLine, Map, MapResolution, Points};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline, Tabs, Wrap};
use ratatui::{Frame, Terminal};
use std::cell::Cell;
use std::collections::VecDeque;
use std::io::{self, Stderr};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

thread_local! {
    static ASCII_RENDER: Cell<bool> = const { Cell::new(false) };
}

fn set_ascii_render(enabled: bool) {
    ASCII_RENDER.with(|value| value.set(enabled));
}

fn is_ascii_render() -> bool {
    ASCII_RENDER.with(Cell::get)
}
const ASCII_BORDER: border::Set<'static> = border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};
const PAGES: [&str; 7] = [
    "F1 FLIGHT",
    "F2 TRAJECTORY",
    "F3 GNC",
    "F4 NAV",
    "F5 VEHICLE",
    "F6 NETWORK",
    "F7 SIM",
];
const EVENT_SEPARATION: u16 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitSystem {
    Si,
    Dual,
    Us,
}
impl UnitSystem {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Si => "SI",
            Self::Dual => "DUAL",
            Self::Us => "US",
        }
    }
    pub const fn next(self) -> Self {
        match self {
            Self::Si => Self::Dual,
            Self::Dual => Self::Us,
            Self::Us => Self::Si,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundProfile {
    Off,
    Cues,
    Cinematic,
}
impl SoundProfile {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Cues => "CUES",
            Self::Cinematic => "CINEMATIC",
        }
    }
    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Cues,
            Self::Cues => Self::Cinematic,
            Self::Cinematic => Self::Off,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    Adaptive,
    Tui,
    Summary,
    None,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotStyle {
    Auto,
    Braille,
    Ascii,
}
impl PlotStyle {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Braille => "BRAILLE",
            Self::Ascii => "ASCII",
        }
    }
    fn marker(self) -> Marker {
        match self {
            Self::Ascii => Marker::Custom('*'),
            Self::Auto | Self::Braille => Marker::Braille,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrajectoryView {
    Ascent,
    Orbit,
    GroundTrack,
}
impl TrajectoryView {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ascent => "ASCENT PROFILE",
            Self::Orbit => "ORBIT VIEW",
            Self::GroundTrack => "GROUND TRACK",
        }
    }
    pub const fn next(self) -> Self {
        match self {
            Self::Ascent => Self::Orbit,
            Self::Orbit => Self::GroundTrack,
            Self::GroundTrack => Self::Ascent,
        }
    }
    pub const fn previous(self) -> Self {
        match self {
            Self::Ascent => Self::GroundTrack,
            Self::Orbit => Self::Ascent,
            Self::GroundTrack => Self::Orbit,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ConsoleConfig {
    pub units: UnitSystem,
    pub sound: SoundProfile,
    pub plot_style: PlotStyle,
    pub trajectory_view: TrajectoryView,
    pub recording: Option<PathBuf>,
    pub title: String,
}
impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            units: UnitSystem::Si,
            sound: SoundProfile::Cues,
            plot_style: PlotStyle::Auto,
            trajectory_view: TrajectoryView::Ascent,
            recording: Some(default_session_path()),
            title: "KSA64 // MISSION CONTROL".into(),
        }
    }
}
#[derive(Debug)]
pub enum ConsoleError {
    Io(io::Error),
    Runner(RunnerError),
    Session(SessionError),
    Worker,
}
impl From<io::Error> for ConsoleError {
    fn from(v: io::Error) -> Self {
        Self::Io(v)
    }
}
impl From<RunnerError> for ConsoleError {
    fn from(v: RunnerError) -> Self {
        Self::Runner(v)
    }
}
impl From<SessionError> for ConsoleError {
    fn from(v: SessionError) -> Self {
        Self::Session(v)
    }
}

enum LiveEvent {
    Update(Box<MissionControlUpdate>),
    Finish(RunnerEvidence),
}
struct ChannelSink {
    tx: Sender<LiveEvent>,
}
impl MissionControlSink for ChannelSink {
    fn publish(&mut self, v: MissionControlUpdate) {
        let _ = self.tx.send(LiveEvent::Update(Box::new(v)));
    }
    fn finish(&mut self, v: &RunnerEvidence) {
        let _ = self.tx.send(LiveEvent::Finish(*v));
    }
}

pub fn run_native_recorded(
    options: RunnerOptions,
    path: PathBuf,
) -> Result<RunnerEvidence, ConsoleError> {
    let control = PaceController::new(options.pace);
    if options.pace == crate::phase6_runner::RunnerPace::Step {
        let c = control.clone();
        thread::spawn(move || {
            let mut line = String::new();
            while io::stdin().read_line(&mut line).is_ok() {
                c.step();
                line.clear();
                if c.snapshot().cancelled {
                    break;
                }
            }
        });
    }
    let mut sink = RecordingSink::create(path)?;
    let evidence = run_native_host_mission_controlled(options, Some(&mut sink), &control)?;
    sink.check()?;
    Ok(evidence)
}
pub fn run_bridge_recorded(
    mut stream: TcpStream,
    max_epochs: u32,
    options: RunnerOptions,
    path: PathBuf,
) -> Result<RunnerEvidence, ConsoleError> {
    let control = PaceController::new(options.pace);
    let mut sink = RecordingSink::create(path)?;
    let evidence = run_world_with_flight_controlled(
        &mut stream,
        max_epochs,
        options,
        Some(&mut sink),
        &control,
    )?;
    sink.check()?;
    Ok(evidence)
}
pub fn run_native_console(
    options: RunnerOptions,
    config: ConsoleConfig,
) -> Result<RunnerEvidence, ConsoleError> {
    let (tx, rx) = mpsc::channel();
    let control = PaceController::new(options.pace);
    let worker_control = control.clone();
    let worker = thread::spawn(move || {
        let mut sink = ChannelSink { tx };
        run_native_host_mission_controlled(options, Some(&mut sink), &worker_control)
    });
    let ui_result = run_live_console(rx, control.clone(), config);
    if ui_result.is_err() {
        control.cancel();
    }
    let evidence = worker.join().map_err(|_| ConsoleError::Worker)??;
    ui_result?;
    Ok(evidence)
}
pub fn run_bridge_console(
    mut stream: TcpStream,
    max_epochs: u32,
    options: RunnerOptions,
    config: ConsoleConfig,
) -> Result<RunnerEvidence, ConsoleError> {
    let (tx, rx) = mpsc::channel();
    let control = PaceController::new(options.pace);
    let worker_control = control.clone();
    let worker = thread::spawn(move || {
        let mut sink = ChannelSink { tx };
        run_world_with_flight_controlled(
            &mut stream,
            max_epochs,
            options,
            Some(&mut sink),
            &worker_control,
        )
    });
    let ui_result = run_live_console(rx, control.clone(), config);
    if ui_result.is_err() {
        control.cancel();
    }
    let evidence = worker.join().map_err(|_| ConsoleError::Worker)??;
    ui_result?;
    Ok(evidence)
}
pub fn run_replay_console(session: Session, mut config: ConsoleConfig) -> Result<(), ConsoleError> {
    config.recording = None;
    let mut app = App::new(config, false, None)?;
    app.history = session.updates;
    app.replay = true;
    app.finished = true;
    for update in &app.history {
        if let Some(aid) = update.aid {
            if aid.events != 0 {
                app.events.push_front(format!(
                    "T+{:07.2}  {}",
                    operational_time(update),
                    event_names(aid.events)
                ));
            }
        }
    }
    while app.events.len() > 32 {
        app.events.pop_back();
    }
    if let Some(first) = app.history.first().copied() {
        app.latest = Some(first);
    }
    run_terminal(&mut app, None)?;
    Ok(())
}

fn run_live_console(
    rx: Receiver<LiveEvent>,
    control: PaceController,
    config: ConsoleConfig,
) -> Result<(), ConsoleError> {
    let mut app = App::new(config, true, Some(control.clone()))?;
    let mut disconnected = false;
    let mut terminal = TerminalGuard::new()?;
    loop {
        for _ in 0..512 {
            match rx.try_recv() {
                Ok(LiveEvent::Update(v)) => app.accept(*v),
                Ok(LiveEvent::Finish(v)) => app.finish(v)?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        terminal.terminal.draw(|f| render(f, &app))?;
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press && app.key(k.code)? {
                    return Ok(());
                }
                if app.detached {
                    drop(terminal);
                    while !app.finished {
                        match rx.recv_timeout(Duration::from_millis(100)) {
                            Ok(LiveEvent::Update(v)) => app.accept(*v),
                            Ok(LiveEvent::Finish(v)) => app.finish(v)?,
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                    return Ok(());
                }
            }
        }
        if app.exit_after_finish && app.finished {
            return Ok(());
        }
        if disconnected && !app.finished {
            return Ok(());
        }
    }
}
fn run_terminal(app: &mut App, control: Option<PaceController>) -> Result<(), ConsoleError> {
    let mut guard = TerminalGuard::new()?;
    let mut last = Instant::now();
    loop {
        if app.replay && app.playing && last.elapsed() >= Duration::from_millis(31) {
            app.cursor = (app.cursor + 1).min(app.history.len().saturating_sub(1));
            app.latest = app.history.get(app.cursor).copied();
            last = Instant::now();
        }
        guard.terminal.draw(|f| render(f, app))?;
        if event::poll(Duration::from_millis(40))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press && app.key(k.code)? {
                    if let Some(c) = control.as_ref() {
                        c.cancel();
                    }
                    return Ok(());
                }
            }
        }
    }
}
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stderr>>,
}
impl TerminalGuard {
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stderr = io::stderr();
        execute!(stderr, EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stderr))?;
        terminal.clear()?;
        Ok(Self { terminal })
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DensityTier {
    Compact,
    Standard,
    Wide,
    Ultra,
}
impl DensityTier {
    fn from_area(area: Rect) -> Self {
        if area.width >= 180 && area.height >= 50 {
            Self::Ultra
        } else if area.width >= 140 && area.height >= 42 {
            Self::Wide
        } else if area.width >= 100 && area.height >= 30 {
            Self::Standard
        } else {
            Self::Compact
        }
    }
}

struct App {
    config: ConsoleConfig,
    audio: AudioEngine,
    page: usize,
    latest: Option<MissionControlUpdate>,
    history: Vec<MissionControlUpdate>,
    plan: Result<PlanReference, crate::phase6_trajectory::PlanError>,
    finished: bool,
    evidence: Option<RunnerEvidence>,
    control: Option<PaceController>,
    recorder: Option<SessionRecorder>,
    record_error: Option<String>,
    record_path: Option<PathBuf>,
    events: VecDeque<String>,
    bookmarks: Vec<u32>,
    freeze: bool,
    detached: bool,
    quit_modal: bool,
    help_modal: bool,
    exit_after_finish: bool,
    live: bool,
    replay: bool,
    cursor: usize,
    playing: bool,
    show_plan: bool,
    show_onboard: bool,
}
impl App {
    fn new(
        config: ConsoleConfig,
        live: bool,
        control: Option<PaceController>,
    ) -> Result<Self, ConsoleError> {
        let record_path = config.recording.clone();
        let recorder = record_path
            .as_ref()
            .map(SessionRecorder::create)
            .transpose()?;
        let audio = AudioEngine::new(config.sound);
        Ok(Self {
            config,
            audio,
            page: 0,
            latest: None,
            history: Vec::new(),
            plan: PlanReference::load_embedded(),
            finished: false,
            evidence: None,
            control,
            recorder,
            record_error: None,
            record_path,
            events: VecDeque::new(),
            bookmarks: Vec::new(),
            freeze: false,
            detached: false,
            quit_modal: false,
            help_modal: false,
            exit_after_finish: false,
            live,
            replay: false,
            cursor: 0,
            playing: false,
            show_plan: true,
            show_onboard: true,
        })
    }
    fn accept(&mut self, v: MissionControlUpdate) {
        if let Some(r) = self.recorder.as_mut() {
            if let Err(error) = r.record_update(v) {
                self.record_error = Some(format!("{error:?}"));
                self.recorder = None;
            }
        }
        self.history.push(v);
        if !self.freeze {
            self.latest = Some(v);
            self.cursor = self.history.len().saturating_sub(1);
        }
        let event_bits = v.aid.map(|a| a.events).unwrap_or(0);
        if event_bits != 0 {
            self.audio.cue(event_bits);
            self.events.push_front(format!(
                "T+{:07.2}  {}",
                operational_time(&v),
                event_names(event_bits)
            ));
            while self.events.len() > 32 {
                self.events.pop_back();
            }
        }
    }
    fn finish(&mut self, v: RunnerEvidence) -> Result<(), ConsoleError> {
        self.finished = true;
        self.audio.completion(v.complete);
        self.evidence = Some(v);
        if let Some(r) = self.recorder.as_mut() {
            r.finish(&v)?
        }
        Ok(())
    }
    fn display_epoch(&self) -> u32 {
        self.latest.map(|v| v.epoch).unwrap_or(0)
    }
    fn prefix(&self) -> &[MissionControlUpdate] {
        let count = self
            .history
            .partition_point(|v| v.epoch <= self.display_epoch());
        &self.history[..count]
    }
    fn key(&mut self, key: KeyCode) -> Result<bool, ConsoleError> {
        if self.help_modal {
            if matches!(key, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')) {
                self.help_modal = false;
            }
            return Ok(false);
        }
        if self.quit_modal {
            return match key {
                KeyCode::Char('s') => {
                    if let Some(c) = self.control.as_ref() {
                        c.cancel()
                    }
                    self.exit_after_finish = true;
                    self.quit_modal = false;
                    Ok(false)
                }
                KeyCode::Char('d') => {
                    self.detached = true;
                    self.quit_modal = false;
                    if let Some(c) = self.control.as_ref() {
                        c.resume()
                    }
                    Ok(false)
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.quit_modal = false;
                    Ok(false)
                }
                _ => Ok(false),
            };
        }
        match key {
            KeyCode::F(n) if (1..=7).contains(&n) => self.page = n as usize - 1,
            KeyCode::Tab if self.page == 1 => {
                self.config.trajectory_view = self.config.trajectory_view.next()
            }
            KeyCode::BackTab if self.page == 1 => {
                self.config.trajectory_view = self.config.trajectory_view.previous()
            }
            KeyCode::Char('1') if self.page == 1 => {
                self.config.trajectory_view = TrajectoryView::Ascent
            }
            KeyCode::Char('2') if self.page == 1 => {
                self.config.trajectory_view = TrajectoryView::Orbit
            }
            KeyCode::Char('3') if self.page == 1 => {
                self.config.trajectory_view = TrajectoryView::GroundTrack
            }
            KeyCode::Char('p') | KeyCode::Char('P') if self.page == 1 => {
                self.show_plan = !self.show_plan
            }
            KeyCode::Char('o') | KeyCode::Char('O') if self.page == 1 => {
                self.show_onboard = !self.show_onboard
            }
            KeyCode::Char('?') => self.help_modal = true,
            KeyCode::Left => {
                if self.replay {
                    self.cursor = self.cursor.saturating_sub(1);
                    self.latest = self.history.get(self.cursor).copied();
                } else {
                    self.page = self.page.saturating_sub(1);
                }
            }
            KeyCode::Right => {
                if self.replay {
                    self.cursor = (self.cursor + 1).min(self.history.len().saturating_sub(1));
                    self.latest = self.history.get(self.cursor).copied();
                } else {
                    self.page = (self.page + 1).min(6);
                }
            }
            KeyCode::Char(' ') => {
                if self.replay {
                    self.playing = !self.playing
                } else if let Some(c) = self.control.as_ref() {
                    c.toggle_pause()
                }
            }
            KeyCode::Char('.') => {
                if let Some(c) = self.control.as_ref() {
                    c.step()
                }
            }
            KeyCode::Char(']') => {
                if let Some(c) = self.control.as_ref() {
                    c.faster()
                }
            }
            KeyCode::Char('[') => {
                if let Some(c) = self.control.as_ref() {
                    c.slower()
                }
            }
            KeyCode::Char('u') | KeyCode::Char('U') => self.config.units = self.config.units.next(),
            KeyCode::Char('s') if !self.quit_modal => {
                self.config.sound = self.config.sound.next();
                self.audio.set_profile(self.config.sound);
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                self.freeze = !self.freeze;
                if !self.freeze {
                    self.latest = self.history.last().copied();
                }
            }
            KeyCode::Char('b') | KeyCode::Char('B') => {
                if let Some(v) = self.latest {
                    self.bookmarks.push(v.epoch);
                    self.events
                        .push_front(format!("BOOKMARK // EPOCH {}", v.epoch));
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => self.export()?,
            KeyCode::Home => {
                self.cursor = 0;
                if self.replay {
                    self.latest = self.history.first().copied();
                }
            }
            KeyCode::End => {
                if self.replay {
                    self.cursor = self.history.len().saturating_sub(1);
                    self.latest = self.history.last().copied();
                }
            }
            KeyCode::Char('q') => {
                if self.finished || !self.live {
                    return Ok(true);
                }
                self.quit_modal = true;
            }
            _ => {}
        }
        Ok(false)
    }
    fn export(&mut self) -> Result<(), ConsoleError> {
        if !self.finished {
            self.events
                .push_front("EXPORT // AVAILABLE POSTFLIGHT".into());
            return Ok(());
        }
        if let Some(path) = self.record_path.as_ref() {
            if path.exists() {
                let s = Session::load(path)?;
                let mut csv = path.clone();
                csv.set_extension("csv");
                s.export_csv(csv)?;
                let mut json = path.clone();
                json.set_extension("json");
                s.export_json(json)?;
                self.events
                    .push_front("EXPORT // CSV + JSON COMPLETE".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct TrackPoint {
    epoch: u32,
    time: f64,
    position: Vec3,
    velocity: Vec3,
    altitude: f64,
    downrange: f64,
    latitude: f64,
    longitude: f64,
    mach: f64,
    q: f64,
    air_speed: f64,
    residual: Residual,
}
fn ground_track(app: &App) -> Vec<TrackPoint> {
    let mut out = Vec::new();
    let mut checksum = None;
    for update in app.prefix() {
        let Some(estimate) = update.ground_estimate else {
            continue;
        };
        if checksum == Some(estimate.checksum) {
            continue;
        }
        checksum = Some(estimate.checksum);
        let position = Vec3::from_q12(estimate.position_q12);
        let velocity = Vec3::from_q24(estimate.velocity_q24);
        let time = estimate.epoch as f64 / FAST_EPOCH_HZ;
        let (latitude, longitude) = latitude_longitude(position, time).unwrap_or((0.0, 0.0));
        let environment =
            environment_from_observed_raw(estimate.position_q12, estimate.velocity_q24)
                .unwrap_or_default();
        let planned = app
            .plan
            .as_ref()
            .ok()
            .and_then(|p| p.point_at_step(estimate.epoch / 4))
            .map(|p| p.position_eci)
            .unwrap_or(position);
        out.push(TrackPoint {
            epoch: estimate.epoch,
            time,
            position,
            velocity,
            altitude: position.norm() - EARTH_RADIUS_KM,
            downrange: great_circle_downrange(position, time),
            latitude,
            longitude,
            mach: environment.mach,
            q: environment.dynamic_pressure_kpa,
            air_speed: environment.air_speed_km_s,
            residual: residual_in_plan_frame(planned, position),
        });
    }
    out
}
fn onboard_track(app: &App) -> Vec<TrackPoint> {
    let mut out = Vec::new();
    for update in app.prefix() {
        let Some(status) = update.status else {
            continue;
        };
        let position = Vec3::from_q12(status.navigation_position_q12);
        let velocity = Vec3::from_q24(status.navigation_velocity_q24);
        let time = status.source_epoch as f64 / FAST_EPOCH_HZ;
        let (latitude, longitude) = latitude_longitude(position, time).unwrap_or((0.0, 0.0));
        let environment = environment_from_observed_raw(
            status.navigation_position_q12,
            status.navigation_velocity_q24,
        )
        .unwrap_or_default();
        let planned = app
            .plan
            .as_ref()
            .ok()
            .and_then(|p| p.point_at_step(status.source_epoch as u32 / 4))
            .map(|p| p.position_eci)
            .unwrap_or(position);
        out.push(TrackPoint {
            epoch: status.source_epoch as u32,
            time,
            position,
            velocity,
            altitude: position.norm() - EARTH_RADIUS_KM,
            downrange: great_circle_downrange(position, time),
            latitude,
            longitude,
            mach: environment.mach,
            q: environment.dynamic_pressure_kpa,
            air_speed: environment.air_speed_km_s,
            residual: residual_in_plan_frame(planned, position),
        });
    }
    out
}
fn latest_ground(app: &App) -> Option<TrackPoint> {
    ground_track(app).last().copied()
}
fn latest_status(app: &App) -> Option<ksa64_interface::phase6::RealtimeStatusCell> {
    app.prefix().iter().rev().find_map(|v| v.status)
}
fn latest_aid(app: &App) -> Option<ksa64_interface::phase6::RealtimeAidCell> {
    app.prefix().iter().rev().find_map(|v| v.aid)
}
fn observed_events(app: &App) -> u16 {
    app.prefix()
        .iter()
        .filter_map(|v| v.aid)
        .fold(0, |bits, aid| bits | aid.events)
}
fn stage_number(app: &App) -> u8 {
    1 + u8::from(observed_events(app) & EVENT_SEPARATION != 0)
}

fn ascent_event_markers(app: &App) -> Vec<(f64, f64)> {
    let mut points = Vec::new();
    for update in app.prefix() {
        if update.aid.is_some_and(|aid| aid.events != 0) {
            if let Some(estimate) = update.ground_estimate {
                let position = Vec3::from_q12(estimate.position_q12);
                let time = estimate.epoch as f64 / FAST_EPOCH_HZ;
                points.push((
                    great_circle_downrange(position, time),
                    position.norm() - EARTH_RADIUS_KM,
                ));
            }
        }
    }
    points
}
fn bookmark_ascent_markers(app: &App) -> Vec<(f64, f64)> {
    let track = ground_track(app);
    app.bookmarks
        .iter()
        .filter_map(|epoch| {
            track
                .iter()
                .rev()
                .find(|p| p.epoch <= *epoch)
                .map(|p| (p.downrange, p.altitude))
        })
        .collect()
}
fn stage_local_step(app: &App) -> u32 {
    if stage_number(app) == 1 {
        return app.display_epoch() / 4;
    }
    let separation = app
        .prefix()
        .iter()
        .find(|v| v.aid.is_some_and(|aid| aid.events & EVENT_SEPARATION != 0))
        .map(|v| v.epoch)
        .unwrap_or(0);
    app.display_epoch().saturating_sub(separation) / 4
}

pub fn render_update_text(
    update: MissionControlUpdate,
    width: u16,
    height: u16,
    page: usize,
) -> Result<String, ConsoleError> {
    render_updates_text(
        &[update],
        width,
        height,
        page,
        TrajectoryView::Ascent,
        PlotStyle::Ascii,
    )
}
pub fn render_updates_text(
    updates: &[MissionControlUpdate],
    width: u16,
    height: u16,
    page: usize,
    view: TrajectoryView,
    plot_style: PlotStyle,
) -> Result<String, ConsoleError> {
    use ratatui::backend::TestBackend;
    let config = ConsoleConfig {
        recording: None,
        sound: SoundProfile::Off,
        trajectory_view: view,
        plot_style,
        ..ConsoleConfig::default()
    };
    let mut app = App::new(config, false, None)?;
    app.page = page.min(6);
    app.history.extend_from_slice(updates);
    app.latest = updates.last().copied();
    app.cursor = updates.len().saturating_sub(1);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, &app)).unwrap();
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            if let Some(cell) = buffer.cell((x, y)) {
                text.push_str(cell.symbol())
            }
        }
        text.push('\n');
    }
    Ok(text)
}

fn render(frame: &mut Frame, app: &App) {
    set_ascii_render(app.config.plot_style == PlotStyle::Ascii);
    let area = frame.area();
    let density = DensityTier::from_area(area);
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);
    render_header(frame, app, root[0]);
    let titles = PAGES.iter().map(|x| Line::from(*x)).collect::<Vec<_>>();
    frame.render_widget(
        Tabs::new(titles)
            .select(app.page)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_set(active_border()),
            )
            .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan).bold())
            .divider(" | "),
        root[1],
    );
    match app.page {
        0 => flight_page(frame, app, root[2], density),
        1 => trajectory_page(frame, app, root[2], density),
        2 => gnc_page(frame, app, root[2], density),
        3 => nav_page(frame, app, root[2], density),
        4 => vehicle_page(frame, app, root[2], density),
        5 => network_page(frame, app, root[2], density),
        _ => sim_page(frame, app, root[2], density),
    }
    render_footer(frame, app, root[3]);
    if app.quit_modal {
        render_quit_modal(frame, area);
    }
    if app.help_modal {
        render_help_modal(frame, area);
    }
}
fn render_header(f: &mut Frame, app: &App, a: Rect) {
    let state = if app.finished {
        "POSTFLIGHT"
    } else if app
        .control
        .as_ref()
        .map(|c| c.snapshot().paused)
        .unwrap_or(false)
    {
        "HOLD"
    } else if app.freeze {
        "DISPLAY FREEZE"
    } else {
        "LIVE"
    };
    let (t, epoch, phase) = app
        .latest
        .map(|v| {
            (
                operational_time(&v),
                v.epoch,
                phase_name(v.inertial.stage_status as u8),
            )
        })
        .unwrap_or((0.0, 0, "AWAITING LINK"));
    let pace = app
        .control
        .as_ref()
        .map(|c| c.snapshot().rate.label())
        .or_else(|| app.latest.map(|v| v.pace.rate.label()))
        .unwrap_or("REPLAY");
    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", app.config.title),
            Style::default().fg(Color::Black).bg(Color::Cyan).bold(),
        ),
        Span::raw(format!(
            "  {state}  T+{t:08.2}  EPOCH {epoch:05}  {phase}  PACE {pace}  {}  PLOT {}",
            app.config.units.label(),
            app.config.plot_style.label()
        )),
        Span::styled(
            app.latest
                .and_then(|v| v.aid)
                .filter(|a| a.events != 0)
                .map(|a| format!("  // {} ", event_names(a.events)))
                .unwrap_or_default(),
            Style::default().fg(Color::Yellow).bold(),
        ),
    ]);
    f.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(active_border())
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        a,
    );
}
fn render_footer(f: &mut Frame, app: &App, a: Rect) {
    let rec = app
        .record_path
        .as_ref()
        .map(|p| format!("REC {}", p.display()))
        .unwrap_or_else(|| "REC OFF".into());
    let keys = if app.page == 1 {
        "TAB VIEW  1/2/3 SELECT  P PLAN  O ONBOARD  ? HELP  F1-F7 CONSOLES  Q QUIT"
    } else if app.replay {
        "LEFT/RIGHT SEEK  SPACE PLAY  HOME/END  ? HELP  E EXPORT  Q EXIT"
    } else {
        "F1-F7 CONSOLES  SPACE HOLD  . STEP  [/] RATE  U UNITS  S SOUND  B MARK  F FREEZE  ? HELP  Q QUIT"
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(keys, Style::default().fg(Color::Cyan)),
            Span::raw("   "),
            Span::styled(rec, Style::default().fg(Color::DarkGray)),
        ]))
        .alignment(Alignment::Center),
        a,
    );
}
fn active_border() -> border::Set<'static> {
    if is_ascii_render() {
        ASCII_BORDER
    } else {
        border::PLAIN
    }
}
fn panel<'a>(title: impl Into<Line<'a>>) -> Block<'a> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_set(active_border())
        .border_style(Style::default().fg(Color::DarkGray))
        .title_style(Style::default().fg(Color::Cyan).bold())
}
fn danger_panel<'a>(title: &'a str) -> Block<'a> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_set(active_border())
        .border_style(Style::default().fg(Color::Red))
        .title_style(Style::default().fg(Color::Red).bold())
}
fn split3(a: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(a)
}
fn metric(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<16}"), Style::default().fg(Color::DarkGray)),
        Span::styled(value, Style::default().fg(Color::White).bold()),
    ])
}
fn badge(label: &str, color: Color) -> Span<'static> {
    Span::styled(
        format!(" {label} "),
        Style::default().fg(Color::Black).bg(color).bold(),
    )
}
fn source_title(name: &str, source: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {name}  "),
            Style::default().fg(Color::Cyan).bold(),
        ),
        badge(source, color),
    ])
}

fn flight_page(f: &mut Frame, app: &App, a: Rect, density: DensityTier) {
    if app.latest.is_none() {
        f.render_widget(
            Paragraph::new("AWAITING KLR6 TELEMETRY")
                .alignment(Alignment::Center)
                .block(panel(" FLIGHT DIRECTOR ")),
            a,
        );
        return;
    }
    let ground = latest_ground(app);
    let status = latest_status(app);
    let aid = latest_aid(app);
    let latest = app.latest.unwrap();
    let rows = if matches!(density, DensityTier::Wide | DensityTier::Ultra) {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(a)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100), Constraint::Percentage(0)])
            .split(a)
    };
    let c = split3(rows[0]);
    let dynamics = if let Some(g) = ground {
        vec![
            metric("ALTITUDE", fmt_alt(g.altitude, app.config.units)),
            metric("SPEED", fmt_speed(g.velocity.norm(), app.config.units)),
            metric("DOWNRANGE", fmt_distance(g.downrange, app.config.units)),
            metric(
                "FLIGHT PATH",
                format!("{:+.3} deg", flight_path_angle(g.position, g.velocity)),
            ),
            metric("MACH", format!("{:.3}", g.mach)),
            metric("DYN PRESSURE", format!("{:.2} kPa", g.q)),
        ]
    } else {
        vec![metric("GROUND SOLUTION", "ACQUIRING".into())]
    };
    f.render_widget(
        Paragraph::new(dynamics).block(panel(source_title(
            "FLIGHT DYNAMICS",
            "GROUND EST",
            Color::Green,
        ))),
        c[0],
    );
    let vehicle = vec![
        metric(
            "PHASE",
            phase_name(latest.inertial.stage_status as u8).into(),
        ),
        metric(
            "ACTIVE STAGE",
            format!("S-{stage}", stage = stage_number(app)),
        ),
        metric(
            "GIMBAL CMD",
            format!(
                "{:+.3} / {:+.3}",
                latest.command.gimbal[0] as f64 / 32768.0,
                latest.command.gimbal[1] as f64 / 32768.0
            ),
        ),
        metric(
            "GIMBAL APPLIED",
            format!(
                "{:+.3} / {:+.3}",
                latest.inertial.gimbal_applied[0] as f64 / 32768.0,
                latest.inertial.gimbal_applied[1] as f64 / 32768.0
            ),
        ),
        metric(
            "RCS PROP",
            aid.map(|x| format!("{:.3} t", x.rcs_propellant_q12 as f64 / 4096.0))
                .unwrap_or_else(|| "-".into()),
        ),
        metric("EVENTS", event_names(observed_events(app))),
    ];
    f.render_widget(
        Paragraph::new(vehicle).block(panel(source_title("VEHICLE", "TELEMETRY", Color::Blue))),
        c[1],
    );
    let go = status.map(|s| s.alarms == 0).unwrap_or(false);
    let track_go = ground.is_some();
    let director = vec![
        metric(
            "FLIGHT COMPUTER",
            if go {
                "GO".into()
            } else {
                "ALARM / WAIT".into()
            },
        ),
        metric(
            "GROUND TRACK",
            if track_go {
                "GO".into()
            } else {
                "ACQUIRING".into()
            },
        ),
        metric(
            "LINK CELLS",
            format!("{} / {}", latest.world_cells, latest.flight_cells),
        ),
        metric("NAV DELTA", format!("{:.3} km", nav_error(&latest))),
        metric(
            "DEADLINES",
            status
                .map(|s| s.deadline_misses.to_string())
                .unwrap_or_else(|| "-".into()),
        ),
        metric(
            "MC ALARMS",
            format!("{:04X}", latest.mission_control_alarms),
        ),
    ];
    f.render_widget(
        Paragraph::new(director).block(panel(" FLIGHT DIRECTOR ")),
        c[2],
    );
    if matches!(density, DensityTier::Wide | DensityTier::Ultra) {
        let lower = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(42),
                Constraint::Percentage(33),
                Constraint::Percentage(25),
            ])
            .split(rows[1]);
        render_ascent_canvas(f, app, lower[0], true);
        render_mission_timeline(f, app, lower[1]);
        render_go_matrix(f, app, lower[2]);
    }
}

fn trajectory_page(f: &mut Frame, app: &App, a: Rect, density: DensityTier) {
    if matches!(density, DensityTier::Ultra) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(a);
        let previews = split3(rows[0]);
        render_ascent_canvas(f, app, previews[0], false);
        render_orbit_canvas(f, app, previews[1], false);
        render_ground_canvas(f, app, previews[2], false);
        render_trajectory_focus(f, app, rows[1], true);
    } else {
        render_trajectory_focus(f, app, a, matches!(density, DensityTier::Wide));
    }
}
fn render_trajectory_focus(f: &mut Frame, app: &App, a: Rect, wide: bool) {
    let columns = if wide || a.width >= 100 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(74), Constraint::Percentage(26)])
            .split(a)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(a)
    };
    match app.config.trajectory_view {
        TrajectoryView::Ascent => render_ascent_canvas(f, app, columns[0], true),
        TrajectoryView::Orbit => render_orbit_canvas(f, app, columns[0], true),
        TrajectoryView::GroundTrack => render_ground_canvas(f, app, columns[0], true),
    }
    render_trajectory_metrics(f, app, columns[1]);
}
fn render_ascent_canvas(f: &mut Frame, app: &App, a: Rect, focus: bool) {
    let plan_points = app
        .plan
        .as_ref()
        .ok()
        .map(|plan| {
            plan.points
                .iter()
                .map(|p| {
                    (
                        great_circle_downrange(p.position_eci, p.time_seconds),
                        p.position_eci.norm() - EARTH_RADIUS_KM,
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ground_points = ground_track(app)
        .into_iter()
        .map(|p| (p.downrange, p.altitude))
        .collect::<Vec<_>>();
    let onboard_points = if app.show_onboard {
        onboard_track(app)
            .into_iter()
            .map(|p| (p.downrange, p.altitude))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let max_x = plan_points
        .iter()
        .chain(&ground_points)
        .map(|p| p.0)
        .fold(100.0, f64::max)
        * 1.05;
    let max_y = plan_points
        .iter()
        .chain(&ground_points)
        .map(|p| p.1)
        .fold(TARGET_APSIS_MAX_KM + 10.0, f64::max)
        .max(TARGET_APSIS_MAX_KM + 10.0);
    let marker = app.config.plot_style.marker();
    let title = if focus {
        format!(" {} // PLAN vs GROUND EST ", TrajectoryView::Ascent.label())
    } else {
        " ASCENT PREVIEW ".into()
    };
    let current = ground_points.last().copied();
    let plan_milestones = app
        .plan
        .as_ref()
        .ok()
        .map(|plan| {
            plan.points
                .iter()
                .filter(|p| p.events != 0)
                .map(|p| {
                    (
                        great_circle_downrange(p.position_eci, p.time_seconds),
                        p.position_eci.norm() - EARTH_RADIUS_KM,
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let event_markers = ascent_event_markers(app);
    let bookmark_markers = bookmark_ascent_markers(app);
    let plan_max_q = app
        .plan
        .as_ref()
        .ok()
        .and_then(|plan| {
            plan.points
                .iter()
                .max_by(|a, b| a.dynamic_pressure_kpa.total_cmp(&b.dynamic_pressure_kpa))
        })
        .map(|p| {
            (
                great_circle_downrange(p.position_eci, p.time_seconds),
                p.position_eci.norm() - EARTH_RADIUS_KM,
            )
        });
    let ground_max_q = ground_track(app)
        .into_iter()
        .max_by(|a, b| a.q.total_cmp(&b.q))
        .map(|p| (p.downrange, p.altitude));
    let canvas = Canvas::default()
        .block(panel(title))
        .marker(marker)
        .x_bounds([0.0, max_x])
        .y_bounds([-5.0, max_y])
        .paint(move |ctx| {
            if app.show_plan {
                draw_polyline(ctx, &plan_points, Color::Cyan);
            }
            draw_polyline(ctx, &ground_points, Color::Green);
            draw_polyline(ctx, &onboard_points, Color::Magenta);
            ctx.draw(&CanvasLine::new(
                0.0,
                TARGET_APSIS_MIN_KM,
                max_x,
                TARGET_APSIS_MIN_KM,
                Color::DarkGray,
            ));
            ctx.draw(&CanvasLine::new(
                0.0,
                TARGET_APSIS_MAX_KM,
                max_x,
                TARGET_APSIS_MAX_KM,
                Color::DarkGray,
            ));
            ctx.draw(&Points {
                coords: &plan_milestones,
                color: Color::Yellow,
            });
            ctx.draw(&Points {
                coords: &event_markers,
                color: Color::Red,
            });
            ctx.draw(&Points {
                coords: &bookmark_markers,
                color: Color::Magenta,
            });
            if let Some(point) = plan_max_q {
                ctx.draw(&Points {
                    coords: &[point],
                    color: Color::Yellow,
                });
            }
            if let Some(point) = ground_max_q {
                ctx.draw(&Points {
                    coords: &[point],
                    color: Color::Green,
                });
            }
            if let Some(point) = current {
                ctx.draw(&Points {
                    coords: &[point],
                    color: Color::White,
                });
            }
        });
    f.render_widget(canvas, a);
}
fn render_orbit_canvas(f: &mut Frame, app: &App, a: Rect, focus: bool) {
    let plan = app.plan.as_ref().ok();
    let basis = plan
        .map(|p| (p.orbit.plane_x, p.orbit.plane_y))
        .unwrap_or((Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)));
    let plan_curve = plan
        .map(|p| {
            sample_orbit(p.orbit, 257)
                .into_iter()
                .map(|v| project_to_plane(v, basis.0, basis.1))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ground = latest_ground(app);
    let estimated_orbit = ground.and_then(|g| orbit_from_state(g.position, g.velocity));
    let estimated_curve = estimated_orbit
        .map(|o| {
            sample_orbit(o, 257)
                .into_iter()
                .map(|v| project_to_plane(v, basis.0, basis.1))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let current = ground.map(|g| project_to_plane(g.position, basis.0, basis.1));
    let plan_apses = plan
        .map(|p| {
            [
                crate::phase6_trajectory::orbit_position_at_true_anomaly(p.orbit, 0.0),
                crate::phase6_trajectory::orbit_position_at_true_anomaly(
                    p.orbit,
                    std::f64::consts::PI,
                ),
            ]
            .into_iter()
            .flatten()
            .map(|v| project_to_plane(v, basis.0, basis.1))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let estimate_apses = estimated_orbit
        .map(|o| {
            [
                crate::phase6_trajectory::orbit_position_at_true_anomaly(o, 0.0),
                crate::phase6_trajectory::orbit_position_at_true_anomaly(o, std::f64::consts::PI),
            ]
            .into_iter()
            .flatten()
            .map(|v| project_to_plane(v, basis.0, basis.1))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let bound = EARTH_RADIUS_KM + TARGET_APSIS_MAX_KM + 180.0;
    let marker = app.config.plot_style.marker();
    let title = if focus {
        " ORBIT VIEW // PLANNED PLANE ".to_owned()
    } else {
        " ORBIT PREVIEW ".to_owned()
    };
    let canvas = Canvas::default()
        .block(panel(title))
        .marker(marker)
        .x_bounds([-bound, bound])
        .y_bounds([-bound, bound])
        .paint(move |ctx| {
            ctx.draw(&Circle::new(0.0, 0.0, EARTH_RADIUS_KM, Color::Blue));
            ctx.draw(&Circle::new(
                0.0,
                0.0,
                EARTH_RADIUS_KM + TARGET_ALTITUDE_KM,
                Color::DarkGray,
            ));
            ctx.draw(&Circle::new(
                0.0,
                0.0,
                EARTH_RADIUS_KM + TARGET_APSIS_MIN_KM,
                Color::DarkGray,
            ));
            ctx.draw(&Circle::new(
                0.0,
                0.0,
                EARTH_RADIUS_KM + TARGET_APSIS_MAX_KM,
                Color::DarkGray,
            ));
            if app.show_plan {
                draw_polyline(ctx, &plan_curve, Color::Cyan);
            }
            draw_polyline(ctx, &estimated_curve, Color::Green);
            ctx.draw(&Points {
                coords: &plan_apses,
                color: Color::Cyan,
            });
            ctx.draw(&Points {
                coords: &estimate_apses,
                color: Color::Yellow,
            });
            if let Some(point) = current {
                ctx.draw(&Points {
                    coords: &[point],
                    color: Color::White,
                });
            }
        });
    f.render_widget(canvas, a);
}
fn render_ground_canvas(f: &mut Frame, app: &App, a: Rect, focus: bool) {
    let mut planned = app
        .plan
        .as_ref()
        .ok()
        .map(|plan| {
            plan.points
                .iter()
                .filter_map(|p| {
                    latitude_longitude(p.position_eci, p.time_seconds).map(|(lat, lon)| (lon, lat))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Ok(plan) = &app.plan {
        let terminal_time = plan.points.last().map(|p| p.time_seconds).unwrap_or(0.0);
        for i in 1..=180 {
            let dt = plan.orbit.period_seconds * i as f64 / 180.0;
            if let Some(position) = propagate_elliptic(plan.orbit, dt) {
                if let Some((lat, lon)) = latitude_longitude(position, terminal_time + dt) {
                    planned.push((lon, lat));
                }
            }
        }
    }
    let ground_points = ground_track(app)
        .into_iter()
        .map(|p| (p.longitude, p.latitude))
        .collect::<Vec<_>>();
    let future = latest_ground(app)
        .and_then(|g| orbit_from_state(g.position, g.velocity).map(|o| (g, o)))
        .map(|(g, o)| {
            let period = o.period_seconds.min(7200.0);
            (0..181)
                .filter_map(|i| {
                    let dt = period * i as f64 / 180.0;
                    propagate_elliptic(o, dt)
                        .and_then(|p| latitude_longitude(p, g.time + dt))
                        .map(|(lat, lon)| (lon, lat))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let plan_segments = split_antimeridian(&planned);
    let ground_segments = split_antimeridian(&ground_points);
    let future_segments = split_antimeridian(&future);
    let current = ground_points.last().copied();
    let marker = app.config.plot_style.marker();
    let resolution = if focus && a.width > 100 {
        MapResolution::High
    } else {
        MapResolution::Low
    };
    let title = if focus {
        " GROUND TRACK // EARTH-FIXED ".to_owned()
    } else {
        " GROUND PREVIEW ".to_owned()
    };
    let canvas = Canvas::default()
        .block(panel(title))
        .marker(marker)
        .x_bounds([-180.0, 180.0])
        .y_bounds([-90.0, 90.0])
        .paint(move |ctx| {
            ctx.draw(&Map {
                resolution,
                color: Color::DarkGray,
            });
            if app.show_plan {
                for segment in &plan_segments {
                    draw_polyline(ctx, segment, Color::Cyan);
                }
            }
            for segment in &future_segments {
                draw_polyline(ctx, segment, Color::Blue);
            }
            for segment in &ground_segments {
                draw_polyline(ctx, segment, Color::Green);
            }
            ctx.draw(&Points {
                coords: &[(LAUNCH_LONGITUDE_DEG, LAUNCH_LATITUDE_DEG)],
                color: Color::Yellow,
            });
            if let Some(point) = current {
                ctx.draw(&Points {
                    coords: &[point],
                    color: Color::White,
                });
            }
        });
    f.render_widget(canvas, a);
}
fn draw_polyline(
    ctx: &mut ratatui::widgets::canvas::Context<'_>,
    points: &[(f64, f64)],
    color: Color,
) {
    for pair in points.windows(2) {
        ctx.draw(&CanvasLine::new(
            pair[0].0, pair[0].1, pair[1].0, pair[1].1, color,
        ));
    }
}
fn render_trajectory_metrics(f: &mut Frame, app: &App, a: Rect) {
    let ground = latest_ground(app);
    let plan = app.plan.as_ref().ok();
    let mut lines = vec![Line::from(vec![
        badge("PLAN", Color::Cyan),
        Span::raw(" "),
        badge("GROUND EST", Color::Green),
        Span::raw(" "),
        badge("ONBOARD", Color::Magenta),
    ])];
    if let Some(g) = ground {
        lines.extend([
            metric("ALTITUDE", fmt_alt(g.altitude, app.config.units)),
            metric("SPEED", fmt_speed(g.velocity.norm(), app.config.units)),
            metric("DOWNRANGE", fmt_distance(g.downrange, app.config.units)),
            metric(
                "LAT / LON",
                format!("{:+.2} / {:+.2} deg", g.latitude, g.longitude),
            ),
            metric("RADIAL DELTA", format!("{:+.3} km", g.residual.radial_km)),
            metric(
                "ALONG DELTA",
                format!("{:+.3} km", g.residual.along_track_km),
            ),
            metric(
                "CROSS DELTA",
                format!("{:+.3} km", g.residual.cross_track_km),
            ),
            metric("FIX EPOCH", g.epoch.to_string()),
        ]);
        if let Some(orbit) = orbit_from_state(g.position, g.velocity) {
            lines.extend(orbit_metrics(orbit));
        }
    } else {
        lines.push(metric("GROUND STATE", "ACQUIRING".into()));
    }
    lines.push(metric(
        "PLAN ID",
        plan.map(|p| format!("KPH5 {:08X}", p.stream_crc32))
            .unwrap_or_else(|| "INVALID".into()),
    ));
    if a.height >= 22 {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(76), Constraint::Percentage(24)])
            .split(a);
        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .block(panel(" ANALYSIS / PROVENANCE ")),
            rows[0],
        );
        let residuals = ground_track(app)
            .into_iter()
            .map(|p| {
                ((p.residual.radial_km.powi(2)
                    + p.residual.along_track_km.powi(2)
                    + p.residual.cross_track_km.powi(2))
                .sqrt()
                    * 1000.0) as u64
            })
            .collect::<Vec<_>>();
        render_series(
            f,
            "PLAN RESIDUAL MAGNITUDE // METRES",
            &residuals,
            Color::Yellow,
            rows[1],
        );
    } else {
        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .block(panel(" ANALYSIS / PROVENANCE ")),
            a,
        );
    }
}
fn orbit_metrics(o: crate::phase6_trajectory::OrbitSolution) -> Vec<Line<'static>> {
    vec![
        metric("ORBIT", format_orbit_kind(o.kind).into()),
        metric("PERIGEE", format!("{:.2} km", o.perigee_altitude_km)),
        metric("APOGEE", format!("{:.2} km", o.apogee_altitude_km)),
        metric("ECCENTRICITY", format!("{:.6}", o.eccentricity)),
        metric("INCLINATION", format!("{:.3} deg", o.inclination_deg)),
        metric(
            "PERIOD",
            if o.period_seconds.is_finite() {
                format!("{:.1} min", o.period_seconds / 60.0)
            } else {
                "-".into()
            },
        ),
        metric(
            "TO APOGEE",
            time_to_apsis(o, true)
                .map(|v| format!("{:.1} s", v))
                .unwrap_or_else(|| "-".into()),
        ),
    ]
}

fn gnc_page(f: &mut Frame, app: &App, a: Rect, density: DensityTier) {
    let Some(v) = app.latest else { return };
    let rows = if matches!(density, DensityTier::Wide | DensityTier::Ultra) {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
            .split(a)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100), Constraint::Percentage(0)])
            .split(a)
    };
    let c = split3(rows[0]);
    let angles = v
        .inertial
        .platform_angle
        .map(|x| x as f64 / 32768.0 * 180.0);
    let rates = v.inertial.angular_rate.map(|x| x as f64 / 32768.0);
    f.render_widget(
        Paragraph::new(vec![
            metric(
                "PLATFORM XYZ",
                format!("{:+.2} {:+.2} {:+.2} deg", angles[0], angles[1], angles[2]),
            ),
            metric(
                "BODY RATES",
                format!("{:+.4} {:+.4} {:+.4}", rates[0], rates[1], rates[2]),
            ),
            metric("DELTA VELOCITY", format!("{:?}", v.inertial.delta_velocity)),
            metric("VALIDITY", format!("{:02X}", v.inertial.validity)),
        ])
        .block(panel(source_title(
            "ATTITUDE / IMU",
            "TELEMETRY",
            Color::Blue,
        ))),
        c[0],
    );
    f.render_widget(
        Paragraph::new(vec![
            metric("GIMBAL COMMAND", format!("{:?}", v.command.gimbal)),
            metric("GIMBAL APPLIED", format!("{:?}", v.inertial.gimbal_applied)),
            metric("RCS COMMAND", format!("{:?}", v.command.rcs)),
            metric("DISCRETE", format!("{:02X}", v.command.discrete)),
            metric("COMMAND STATUS", format!("{:02X}", v.command.status)),
        ])
        .block(panel(source_title(
            "CONTROL",
            "COMMAND+FEEDBACK",
            Color::Magenta,
        ))),
        c[1],
    );
    f.render_widget(
        Paragraph::new(vec![
            metric("GUIDE START", format!("{:?}", v.guidance.start)),
            metric("GUIDE END", format!("{:?}", v.guidance.end)),
            metric("GUIDE RATE", format!("{:?}", v.guidance.rate)),
            metric("SOURCE EPOCH", v.command.source_epoch.to_string()),
            metric("EFFECTIVE", v.command.effective_epoch.to_string()),
        ])
        .block(panel(" GUIDANCE SLICE ")),
        c[2],
    );
    if matches!(density, DensityTier::Wide | DensityTier::Ultra) {
        let lower = split3(rows[1]);
        render_rate_history(f, app, lower[0]);
        render_gimbal_history(f, app, lower[1]);
        render_guidance_timeline(f, app, lower[2]);
    }
}

fn render_series(f: &mut Frame, title: &str, data: &[u64], color: Color, area: Rect) {
    if is_ascii_render() {
        let width = area.width.saturating_sub(2) as usize;
        let maximum = data.iter().copied().max().unwrap_or(1).max(1);
        let chars = b" .:-=+*#%@";
        let mut output = String::with_capacity(width);
        for column in 0..width {
            let index = if width <= 1 {
                data.len().saturating_sub(1)
            } else {
                column.saturating_mul(data.len().saturating_sub(1)) / (width - 1)
            };
            let value = data.get(index).copied().unwrap_or(0);
            let level = (value.saturating_mul((chars.len() - 1) as u64) / maximum) as usize;
            output.push(chars[level] as char);
        }
        f.render_widget(
            Paragraph::new(output)
                .style(Style::default().fg(color))
                .block(panel(format!(" {title} "))),
            area,
        );
    } else {
        f.render_widget(
            Sparkline::default()
                .block(panel(format!(" {title} ")))
                .data(data)
                .style(Style::default().fg(color)),
            area,
        );
    }
}

fn render_progress(
    f: &mut Frame,
    title: &str,
    label: String,
    ratio: f64,
    color: Color,
    area: Rect,
) {
    let ratio = ratio.clamp(0.0, 1.0);
    if is_ascii_render() {
        let width = area.width.saturating_sub(4) as usize;
        let filled = (ratio * width as f64).round() as usize;
        let bar = format!(
            "[{}{}] {}",
            "#".repeat(filled.min(width)),
            "-".repeat(width.saturating_sub(filled)),
            label
        );
        f.render_widget(
            Paragraph::new(bar)
                .style(Style::default().fg(color))
                .block(panel(format!(" {title} "))),
            area,
        );
    } else {
        f.render_widget(
            Gauge::default()
                .block(panel(format!(" {title} ")))
                .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
                .ratio(ratio)
                .label(label),
            area,
        );
    }
}

fn render_rate_history(f: &mut Frame, app: &App, a: Rect) {
    let data = app
        .prefix()
        .iter()
        .map(|v| {
            v.inertial
                .angular_rate
                .iter()
                .map(|x| x.unsigned_abs() as u64)
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    render_series(f, "BODY-RATE MAGNITUDE", &data, Color::Green, a)
}
fn render_gimbal_history(f: &mut Frame, app: &App, a: Rect) {
    let data = app
        .prefix()
        .iter()
        .map(|v| {
            (i32::from(v.command.gimbal[0]) - i32::from(v.inertial.gimbal_applied[0]))
                .unsigned_abs() as u64
        })
        .collect::<Vec<_>>();
    render_series(f, "COMMAND-FEEDBACK ERROR", &data, Color::Magenta, a)
}
fn render_guidance_timeline(f: &mut Frame, app: &App, a: Rect) {
    let items = app
        .prefix()
        .iter()
        .rev()
        .step_by(256)
        .take(8)
        .map(|v| {
            ListItem::new(format!(
                "E{:05} {:?} -> {:?}",
                v.epoch, v.guidance.start, v.guidance.end
            ))
        })
        .collect::<Vec<_>>();
    f.render_widget(List::new(items).block(panel(" GUIDANCE TIMELINE ")), a)
}

fn nav_page(f: &mut Frame, app: &App, a: Rect, density: DensityTier) {
    let c = split3(a);
    let onboard = latest_status(app);
    let ground = app.latest.and_then(|v| v.ground_estimate);
    let make = |x: Option<([i32; 3], [i32; 3])>| {
        if let Some((p, v)) = x {
            vec![
                metric("POSITION Q12", format!("{:?}", p)),
                metric("VELOCITY Q24", format!("{:?}", v)),
            ]
        } else {
            vec![metric("STATE", "NO SOLUTION".into())]
        }
    };
    f.render_widget(
        Paragraph::new(make(
            onboard.map(|s| (s.navigation_position_q12, s.navigation_velocity_q24)),
        ))
        .block(panel(source_title(
            "ONBOARD NAVIGATION",
            "ONBOARD",
            Color::Magenta,
        ))),
        c[0],
    );
    f.render_widget(
        Paragraph::new(make(ground.map(|s| (s.position_q12, s.velocity_q24)))).block(panel(
            source_title("GROUND SOLUTION", "GROUND EST", Color::Green),
        )),
        c[1],
    );
    let latest = app.latest;
    let cmp = latest
        .and_then(|v| v.comparison)
        .map(|x| {
            vec![
                metric("POSITION DELTA", format!("{:?}", x.position_delta_q12)),
                metric("VELOCITY DELTA", format!("{:?}", x.velocity_delta_q24)),
                metric(
                    "POSITION NORM",
                    format!("{:.6} km", latest.map(|v| nav_error(&v)).unwrap_or(0.0)),
                ),
                metric(
                    "FIX AGE",
                    ground
                        .map(|g| format!("{} epochs", app.display_epoch().saturating_sub(g.epoch)))
                        .unwrap_or_else(|| "-".into()),
                ),
                metric(
                    "FIX COUNT",
                    ground
                        .map(|g| g.fixes.to_string())
                        .unwrap_or_else(|| "0".into()),
                ),
            ]
        })
        .unwrap_or_else(|| vec![metric("COMPARISON", "PENDING".into())]);
    if matches!(density, DensityTier::Ultra) {
        f.render_widget(
            Paragraph::new(cmp).block(panel(" INDEPENDENT CROSSCHECK / RESIDUALS ")),
            c[2],
        )
    } else {
        let nav = ground_track(app)
            .into_iter()
            .map(|p| {
                ((p.residual.radial_km.powi(2)
                    + p.residual.along_track_km.powi(2)
                    + p.residual.cross_track_km.powi(2))
                .sqrt()
                    * 1000.0) as u64
            })
            .collect::<Vec<_>>();
        render_series(f, "NAV RESIDUAL HISTORY", &nav, Color::Yellow, c[2]);
    }
}

fn vehicle_page(f: &mut Frame, app: &App, a: Rect, density: DensityTier) {
    let Some(v) = app.latest else { return };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(a);
    let aid = latest_aid(app);
    let stage = stage_number(app);
    let stage_config = ksa64_sim::phase5_vehicle::ksa5a_stage(stage - 1);
    let progress = stage_config
        .map(|s| {
            (stage_local_step(app) as f64 / s.burn_mission_steps.max(1) as f64).clamp(0.0, 1.0)
        })
        .unwrap_or(0.0);
    render_progress(
        f,
        "PLAN-DERIVED ACTIVE-STAGE TIMELINE // MODEL EST",
        format!("S-{stage} {:.1}% TIMELINE", progress * 100.0),
        progress,
        Color::Cyan,
        rows[0],
    );
    let c = split3(rows[1]);
    f.render_widget(
        Paragraph::new(vec![
            metric("STACK", "[PAYLOAD]-[S2]-[S1]".into()),
            metric("ACTIVE STAGE", format!("S-{stage}")),
            metric("PHASE", phase_name(v.inertial.stage_status as u8).into()),
            metric("STAGE STATUS", format!("{:04X}", v.inertial.stage_status)),
            metric("EVENT UNION", event_names(observed_events(app))),
        ])
        .block(panel(source_title(
            "VEHICLE STACK",
            "TELEMETRY+PLAN",
            Color::Blue,
        ))),
        c[0],
    );
    f.render_widget(
        Paragraph::new(vec![
            metric(
                "RCS PROP",
                aid.map(|x| format!("{:.4} t", x.rcs_propellant_q12 as f64 / 4096.0))
                    .unwrap_or_else(|| "-".into()),
            ),
            metric("RCS COMMAND", format!("{:?}", v.command.rcs)),
            metric("GIMBAL CMD", format!("{:?}", v.command.gimbal)),
            metric("GIMBAL APPLIED", format!("{:?}", v.inertial.gimbal_applied)),
            metric("DISCRETE", format!("{:02X}", v.command.discrete)),
        ])
        .block(panel(source_title(
            "PROPULSION / ACTUATION",
            "TELEMETRY",
            Color::Blue,
        ))),
        c[1],
    );
    let ground = latest_ground(app);
    f.render_widget(
        Paragraph::new(vec![
            metric(
                "MACH",
                ground
                    .map(|g| format!("{:.3}", g.mach))
                    .unwrap_or_else(|| "-".into()),
            ),
            metric(
                "DYNAMIC PRESS",
                ground
                    .map(|g| format!("{:.3} kPa", g.q))
                    .unwrap_or_else(|| "-".into()),
            ),
            metric(
                "AIR SPEED",
                ground
                    .map(|g| format!("{:.4} km/s", g.air_speed))
                    .unwrap_or_else(|| "-".into()),
            ),
            metric("SOURCE", "GROUND STATE + MODEL".into()),
            metric(
                "STRUCTURE",
                if matches!(density, DensityTier::Ultra) {
                    "SEE F7 TRUTH".into()
                } else {
                    "F7".into()
                },
            ),
        ])
        .block(panel(source_title(
            "LOAD ESTIMATE",
            "MODEL EST",
            Color::Yellow,
        ))),
        c[2],
    );
}

fn network_page(f: &mut Frame, app: &App, a: Rect, density: DensityTier) {
    let Some(v) = app.latest else { return };
    let rows = if matches!(density, DensityTier::Wide | DensityTier::Ultra) {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(a)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100), Constraint::Percentage(0)])
            .split(a)
    };
    let c = split3(rows[0]);
    f.render_widget(
        Paragraph::new(vec![
            metric("WORLD CELLS", v.world_cells.to_string()),
            metric("FLIGHT CELLS", v.flight_cells.to_string()),
            metric("TRANSCRIPT", format!("{:08X}", v.transcript_checksum)),
            metric("MC ALARMS", format!("{:04X}", v.mission_control_alarms)),
            metric(
                "RUN WALL",
                format!("{:.3} s", v.wall_micros as f64 / 1_000_000.0),
            ),
        ])
        .block(panel(" KLR6 NETWORK ")),
        c[0],
    );
    let fix = v.ground_fix;
    let estimate = v.ground_estimate;
    f.render_widget(
        Paragraph::new(vec![
            metric(
                "FIX ID",
                fix.map(|x| x.fix_id.to_string())
                    .unwrap_or_else(|| "-".into()),
            ),
            metric(
                "FIX COUNT",
                estimate
                    .map(|x| x.fixes.to_string())
                    .unwrap_or_else(|| "0".into()),
            ),
            metric(
                "GROUND CRC",
                estimate
                    .map(|x| format!("{:08X}", x.checksum))
                    .unwrap_or_else(|| "-".into()),
            ),
            metric(
                "DELIVERY LAG",
                fix.map(|x| format!("{} epochs", x.production_epoch - x.measurement_epoch))
                    .unwrap_or_else(|| "-".into()),
            ),
            metric(
                "FIX AGE",
                estimate
                    .map(|x| format!("{} epochs", v.epoch.saturating_sub(x.epoch)))
                    .unwrap_or_else(|| "-".into()),
            ),
        ])
        .block(panel(source_title(
            "TRACKING NETWORK",
            "GROUND EST",
            Color::Green,
        ))),
        c[1],
    );
    let items = app
        .events
        .iter()
        .take(12)
        .map(|x| ListItem::new(x.as_str()))
        .collect::<Vec<_>>();
    f.render_widget(List::new(items).block(panel(" EVENT LOG ")), c[2]);
    if matches!(density, DensityTier::Wide | DensityTier::Ultra) {
        let lower = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(rows[1]);
        render_tracking_ribbon(f, app, lower[0]);
        render_alarm_grid(f, app, lower[1]);
    }
}
fn render_tracking_ribbon(f: &mut Frame, app: &App, a: Rect) {
    let data = app
        .prefix()
        .iter()
        .map(|v| {
            if v.ground_fix.is_some() {
                100
            } else if v.ground_estimate.is_some() {
                35
            } else {
                0
            }
        })
        .collect::<Vec<u64>>();
    render_series(
        f,
        "TRACKING AVAILABILITY // FIX=HIGH HOLD=LOW",
        &data,
        Color::Green,
        a,
    )
}
fn render_alarm_grid(f: &mut Frame, app: &App, a: Rect) {
    let status = latest_status(app);
    let latest = app.latest;
    let rows = vec![
        go_line("FLIGHT", status.map(|s| s.alarms == 0).unwrap_or(false)),
        go_line(
            "MISSION CONTROL",
            latest
                .map(|v| v.mission_control_alarms == 0)
                .unwrap_or(false),
        ),
        go_line("TRACKING", latest.and_then(|v| v.ground_estimate).is_some()),
        go_line(
            "DEADLINES",
            status.map(|s| s.deadline_misses == 0).unwrap_or(false),
        ),
        go_line("RECORDING", app.record_error.is_none()),
    ];
    f.render_widget(Paragraph::new(rows).block(panel(" ALARM ANNUNCIATOR ")), a)
}
fn go_line(name: &str, go: bool) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{name:<20}"), Style::default().fg(Color::DarkGray)),
        badge(
            if go { "GO" } else { "NO-GO" },
            if go { Color::Green } else { Color::Red },
        ),
    ])
}

fn sim_page(f: &mut Frame, app: &App, a: Rect, density: DensityTier) {
    let Some(v) = app.latest else { return };
    let rows = if matches!(density, DensityTier::Wide | DensityTier::Ultra) {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(a)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100), Constraint::Percentage(0)])
            .split(a)
    };
    let c = split3(rows[0]);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(badge("OMNISCIENT - NON-OPERATIONAL", Color::Red)),
            metric("TRUE POSITION", format!("{:?}", v.director.position_q12)),
            metric("TRUE VELOCITY", format!("{:?}", v.director.velocity_q24)),
            metric("TRUE ACCEL", format!("{:?}", v.director.acceleration_q28)),
            metric("ATTITUDE Q30", format!("{:?}", v.director.attitude_q30)),
            metric(
                "BODY RATE Q24",
                format!("{:?}", v.director.angular_rate_q24),
            ),
        ])
        .block(danger_panel("SIM DIRECTOR // TRUTH")),
        c[0],
    );
    f.render_widget(
        Paragraph::new(vec![
            metric(
                "TOTAL MASS",
                format!("{:.3} t", v.director.total_mass_q12 as f64 / 4096.0),
            ),
            metric(
                "ACTIVE PROP",
                format!("{:.3} t", v.director.active_propellant_q12 as f64 / 4096.0),
            ),
            metric(
                "RCS PROP",
                format!("{:.3} t", v.director.rcs_propellant_q12 as f64 / 4096.0),
            ),
            metric(
                "MACH",
                format!("{:.3}", v.director.mach_q16 as f64 / 65536.0),
            ),
            metric(
                "DYN PRESSURE",
                format!(
                    "{:.3} kPa",
                    v.director.dynamic_pressure_q16 as f64 / 65536.0
                ),
            ),
            metric(
                "AOA SINE",
                format!(
                    "{:+.5}",
                    v.director.angle_of_attack_sine_q16 as f64 / 65536.0
                ),
            ),
            metric("FLEX MODES", format!("{:?}", v.director.flexible_q24)),
        ])
        .block(danger_panel("TRUE VEHICLE / LOADS")),
        c[1],
    );
    let truth = orbit_from_state(
        Vec3::from_q12(v.director.position_q12),
        Vec3::from_q24(v.director.velocity_q24),
    );
    let mut ops = vec![
        metric("BOOKMARKS", app.bookmarks.len().to_string()),
        metric("FREEZE", app.freeze.to_string()),
        metric(
            "RECORDING",
            if app.record_error.is_some() {
                "FAULT".into()
            } else if app.recorder.is_some() {
                "ACTIVE".into()
            } else {
                "OFF".into()
            },
        ),
        metric(
            "OPERATOR",
            if app.detached {
                "DETACHED".into()
            } else {
                "CONSOLE".into()
            },
        ),
        metric(
            "RUN",
            if app.finished {
                "COMPLETE".into()
            } else {
                "ACTIVE".into()
            },
        ),
    ];
    if let Some(o) = truth {
        ops.extend(orbit_metrics(o));
    }
    f.render_widget(
        Paragraph::new(ops).block(danger_panel("SIM OPERATIONS / TRUE ORBIT")),
        c[2],
    );
    if matches!(density, DensityTier::Wide | DensityTier::Ultra) {
        let lower = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(rows[1]);
        render_truth_orbit(f, app, lower[0]);
        render_truth_errors(f, app, lower[1]);
    }
}
fn render_truth_orbit(f: &mut Frame, app: &App, a: Rect) {
    let Some(v) = app.latest else { return };
    let truth = orbit_from_state(
        Vec3::from_q12(v.director.position_q12),
        Vec3::from_q24(v.director.velocity_q24),
    );
    let plan = app.plan.as_ref().ok();
    let basis = plan
        .map(|p| (p.orbit.plane_x, p.orbit.plane_y))
        .unwrap_or((Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)));
    let curve = truth
        .map(|o| {
            sample_orbit(o, 257)
                .into_iter()
                .map(|p| project_to_plane(p, basis.0, basis.1))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let bound = EARTH_RADIUS_KM + 400.0;
    let canvas = Canvas::default()
        .block(danger_panel("TRUE ORBIT // F7 ONLY"))
        .marker(app.config.plot_style.marker())
        .x_bounds([-bound, bound])
        .y_bounds([-bound, bound])
        .paint(move |ctx| {
            ctx.draw(&Circle::new(0.0, 0.0, EARTH_RADIUS_KM, Color::Blue));
            draw_polyline(ctx, &curve, Color::Red);
        });
    f.render_widget(canvas, a)
}
fn render_truth_errors(f: &mut Frame, app: &App, a: Rect) {
    let Some(v) = app.latest else { return };
    let truth = Vec3::from_q12(v.director.position_q12);
    let ground = v.ground_estimate.map(|g| Vec3::from_q12(g.position_q12));
    let onboard = latest_status(app).map(|s| Vec3::from_q12(s.navigation_position_q12));
    f.render_widget(
        Paragraph::new(vec![
            Line::from(badge("SIM TRUTH", Color::Red)),
            metric(
                "GROUND ERROR",
                ground
                    .map(|p| format!("{:.6} km", (p - truth).norm()))
                    .unwrap_or_else(|| "-".into()),
            ),
            metric(
                "ONBOARD ERROR",
                onboard
                    .map(|p| format!("{:.6} km", (p - truth).norm()))
                    .unwrap_or_else(|| "-".into()),
            ),
            metric(
                "GIMBAL REQUEST",
                format!("{:?}", v.director.gimbal_requested_q16),
            ),
            metric(
                "GIMBAL LAGGED",
                format!("{:?}", v.director.gimbal_lagged_q16),
            ),
            metric(
                "GIMBAL APPLIED",
                format!("{:?}", v.director.gimbal_applied_q16),
            ),
        ])
        .block(danger_panel("TRUTH RESIDUALS / ACTUATOR INTERNALS")),
        a,
    )
}

fn render_mission_timeline(f: &mut Frame, app: &App, a: Rect) {
    let terminal = app
        .plan
        .as_ref()
        .ok()
        .map(|p| p.points.last().map(|x| x.time_seconds).unwrap_or(400.0))
        .unwrap_or(400.0);
    let now = app.display_epoch() as f64 / FAST_EPOCH_HZ;
    let ratio = (now / terminal).clamp(0.0, 1.0);
    render_progress(
        f,
        "MISSION TIMELINE // EVENTS + BOOKMARKS",
        format!(
            "T+{now:.1}s / PLAN {terminal:.1}s  {}",
            event_names(observed_events(app))
        ),
        ratio,
        Color::Cyan,
        a,
    )
}
fn render_go_matrix(f: &mut Frame, app: &App, a: Rect) {
    let status = latest_status(app);
    let latest = app.latest;
    let rows = vec![
        go_line("FLIGHT", status.map(|s| s.alarms == 0).unwrap_or(false)),
        go_line("GUIDANCE", latest.is_some()),
        go_line("NAVIGATION", status.is_some()),
        go_line("TRACKING", latest.and_then(|v| v.ground_estimate).is_some()),
        go_line(
            "COMM",
            latest
                .map(|v| v.mission_control_alarms == 0)
                .unwrap_or(false),
        ),
    ];
    f.render_widget(Paragraph::new(rows).block(panel(" GO / NO-GO ")), a)
}

fn render_quit_modal(f: &mut Frame, a: Rect) {
    let w = a.width.min(64);
    let h = 9;
    let r = Rect {
        x: a.x + (a.width - w) / 2,
        y: a.y + (a.height - h) / 2,
        width: w,
        height: h,
    };
    f.render_widget(ratatui::widgets::Clear, r);
    f.render_widget(
        Paragraph::new(vec![
            Line::from("MISSION IS STILL RUNNING"),
            Line::from(""),
            Line::from("[S] STOP RUN   [D] DETACH / CONTINUE HEADLESS"),
            Line::from("[ESC/Q] RETURN TO CONSOLE"),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(danger_panel("FLIGHT DIRECTOR CONFIRMATION")),
        r,
    )
}
fn render_help_modal(f: &mut Frame, a: Rect) {
    let w = a.width.min(92);
    let h = a.height.min(24);
    let r = Rect {
        x: a.x + (a.width - w) / 2,
        y: a.y + (a.height - h) / 2,
        width: w,
        height: h,
    };
    f.render_widget(ratatui::widgets::Clear, r);
    let text = vec![
        Line::from("GLOBAL  F1-F7 consoles | U units | S sound | B bookmark | F freeze | Q quit"),
        Line::from("PACE    Space hold/play | . single step | [ slower | ] faster"),
        Line::from(
            "F2      Tab/Shift+Tab cycle | 1 ascent | 2 orbit | 3 ground | P plan | O onboard",
        ),
        Line::from("REPLAY  Left/Right seek | Home/End jump | Space play/pause | E export"),
        Line::from(""),
        Line::from(vec![
            badge("PLAN", Color::Cyan),
            Span::raw(" frozen reviewed nominal reference"),
        ]),
        Line::from(vec![
            badge("GROUND EST", Color::Green),
            Span::raw(" independent tracking solution"),
        ]),
        Line::from(vec![
            badge("ONBOARD", Color::Magenta),
            Span::raw(" flight-computer navigation"),
        ]),
        Line::from(vec![
            badge("MODEL EST", Color::Yellow),
            Span::raw(" derived on the ground from observed state"),
        ]),
        Line::from(vec![
            badge("SIM TRUTH", Color::Red),
            Span::raw(" omniscient and restricted to F7"),
        ]),
        Line::from(""),
        Line::from(format!(
            "PLAN KPH5 CRC {:08X} | Earth radius {:.3} km | target 200 km / 51.6 deg",
            PLAN_STREAM_CRC32, EARTH_RADIUS_KM
        )),
        Line::from("Press ? or Esc to close"),
    ];
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(panel(" MISSION CONTROL HELP / DATA PROVENANCE ")),
        r,
    )
}

fn operational_time(v: &MissionControlUpdate) -> f64 {
    v.epoch as f64 / FAST_EPOCH_HZ
}
fn nav_error(v: &MissionControlUpdate) -> f64 {
    v.comparison
        .map(|c| {
            c.position_delta_q12
                .iter()
                .map(|x| (*x as f64 / 4096.0).powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .unwrap_or(0.0)
}
fn phase_name(p: u8) -> &'static str {
    match p {
        0 => "COAST / PRE-IGNITION",
        1 => "BURNING",
        2 => "COAST / PRE-SEPARATION",
        3 => "COMPLETE",
        _ => "UNKNOWN",
    }
}
fn event_names(e: u16) -> String {
    if e == 0 {
        return "NONE".into();
    }
    let names = [
        (1, "IGNITION"),
        (2, "CUTOFF"),
        (4, "SEPARATION"),
        (8, "ABORT"),
        (16, "GPS OUTAGE"),
        (32, "STAR OUTAGE"),
        (64, "GIMBAL JAM"),
        (128, "DAMPING LOSS"),
        (256, "RCS DEPLETED"),
    ];
    names
        .iter()
        .filter_map(|(bit, name)| (e & bit != 0).then_some(*name))
        .collect::<Vec<_>>()
        .join("+")
}
fn fmt_alt(km: f64, u: UnitSystem) -> String {
    match u {
        UnitSystem::Si => format!("{km:.3} km"),
        UnitSystem::Us => format!("{:.3} mi", km * 0.621371),
        UnitSystem::Dual => format!("{km:.3} km / {:.2} mi", km * 0.621371),
    }
}
fn fmt_distance(km: f64, u: UnitSystem) -> String {
    fmt_alt(km, u)
}
fn fmt_speed(kms: f64, u: UnitSystem) -> String {
    match u {
        UnitSystem::Si => format!("{kms:.4} km/s"),
        UnitSystem::Us => format!("{:.1} mph", kms * 2236.936),
        UnitSystem::Dual => format!("{kms:.4} km/s / {:.0} mph", kms * 2236.936),
    }
}
