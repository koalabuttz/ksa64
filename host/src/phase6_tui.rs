//! Ratatui presentation layer for live and recorded Phase 6 Mission Control.
use crate::phase6_audio::AudioEngine;
use crate::phase6_runner::{
    run_native_host_mission_controlled, run_world_with_flight_controlled, MissionControlSink,
    MissionControlUpdate, PaceController, RunnerError, RunnerEvidence, RunnerOptions,
};
use crate::phase6_session::{
    default_session_path, RecordingSink, Session, SessionError, SessionRecorder,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline, Tabs, Wrap};
use ratatui::{Frame, Terminal};
use std::collections::VecDeque;
use std::io::{self, Stderr};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

const EARTH_RADIUS_KM: f64 = 6371.0;
const HISTORY: usize = 2048;
const PAGES: [&str; 7] = [
    "F1 FLIGHT",
    "F2 TRAJECTORY",
    "F3 GNC",
    "F4 NAV",
    "F5 VEHICLE",
    "F6 NETWORK",
    "F7 SIM",
];

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
#[derive(Clone, Debug)]
pub struct ConsoleConfig {
    pub units: UnitSystem,
    pub sound: SoundProfile,
    pub recording: Option<PathBuf>,
    pub title: String,
}
impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            units: UnitSystem::Si,
            sound: SoundProfile::Cues,
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
    app.replay = Some(session.updates);
    if let Some(items) = app.replay.as_ref() {
        if let Some(first) = items.first().copied() {
            app.accept(first)
        }
    }
    app.finished = true;
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
                Ok(LiveEvent::Finish(v)) => {
                    app.finish(v)?;
                }
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
        if let Some(replay) = app.replay.as_ref() {
            if app.playing && last.elapsed() >= Duration::from_millis(31) {
                app.cursor = (app.cursor + 1).min(replay.len().saturating_sub(1));
                let v = replay[app.cursor];
                app.accept(v);
                last = Instant::now();
            }
        }
        guard.terminal.draw(|f| render(f, app))?;
        if event::poll(Duration::from_millis(40))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press && app.key(k.code)? {
                    if let Some(c) = control.as_ref() {
                        c.cancel()
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

struct App {
    config: ConsoleConfig,
    audio: AudioEngine,
    page: usize,
    latest: Option<MissionControlUpdate>,
    finished: bool,
    evidence: Option<RunnerEvidence>,
    control: Option<PaceController>,
    recorder: Option<SessionRecorder>,
    record_error: Option<String>,
    record_path: Option<PathBuf>,
    alt: Vec<u64>,
    speed: Vec<u64>,
    q: Vec<u64>,
    nav_error: Vec<u64>,
    events: VecDeque<String>,
    bookmarks: Vec<u32>,
    freeze: bool,
    detached: bool,
    quit_modal: bool,
    exit_after_finish: bool,
    live: bool,
    replay: Option<Vec<MissionControlUpdate>>,
    cursor: usize,
    playing: bool,
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
            finished: false,
            evidence: None,
            control,
            recorder,
            record_error: None,
            record_path,
            alt: Vec::new(),
            speed: Vec::new(),
            q: Vec::new(),
            nav_error: Vec::new(),
            events: VecDeque::new(),
            bookmarks: Vec::new(),
            freeze: false,
            detached: false,
            quit_modal: false,
            exit_after_finish: false,
            live,
            replay: None,
            cursor: 0,
            playing: false,
        })
    }
    fn accept(&mut self, v: MissionControlUpdate) {
        if let Some(r) = self.recorder.as_mut() {
            if let Err(error) = r.record_update(v) {
                self.record_error = Some(format!("{error:?}"));
                self.recorder = None;
            }
        }
        if !self.freeze {
            self.latest = Some(v);
        }
        push(&mut self.alt, ((altitude(&v) * 10.0).max(0.0)) as u64);
        push(&mut self.speed, (speed(&v) * 1000.0) as u64);
        push(&mut self.q, (q(&v).max(0.0)) as u64);
        push(&mut self.nav_error, (nav_error(&v) * 1000.0) as u64);
        if v.director.events != 0 {
            self.audio.cue(v.director.events);
            self.events.push_front(format!(
                "T+{:07.2}  {}",
                time(&v),
                event_names(v.director.events)
            ));
            while self.events.len() > 12 {
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
    fn key(&mut self, key: KeyCode) -> Result<bool, ConsoleError> {
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
            KeyCode::Left => {
                if let Some(items) = self.replay.as_ref() {
                    self.cursor = self.cursor.saturating_sub(1);
                    if let Some(v) = items.get(self.cursor).copied() {
                        self.latest = Some(v)
                    }
                } else {
                    self.page = self.page.saturating_sub(1)
                }
            }
            KeyCode::Right => {
                if let Some(items) = self.replay.as_ref() {
                    self.cursor = (self.cursor + 1).min(items.len().saturating_sub(1));
                    if let Some(v) = items.get(self.cursor).copied() {
                        self.latest = Some(v)
                    }
                } else {
                    self.page = (self.page + 1).min(6)
                }
            }
            KeyCode::Char(' ') => {
                if self.replay.is_some() {
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
            KeyCode::Char('u') => self.config.units = self.config.units.next(),
            KeyCode::Char('s') => {
                self.config.sound = self.config.sound.next();
                self.audio.set_profile(self.config.sound);
            }
            KeyCode::Char('f') => self.freeze = !self.freeze,
            KeyCode::Char('b') => {
                if let Some(v) = self.latest {
                    self.bookmarks.push(v.epoch);
                    self.events
                        .push_front(format!("BOOKMARK // EPOCH {}", v.epoch))
                }
            }
            KeyCode::Char('e') => self.export()?,
            KeyCode::Home => {
                self.cursor = 0;
                if let Some(v) = self.replay.as_ref().and_then(|x| x.first()).copied() {
                    self.latest = Some(v)
                }
            }
            KeyCode::End => {
                if let Some(items) = self.replay.as_ref() {
                    self.cursor = items.len().saturating_sub(1);
                    self.latest = items.last().copied()
                }
            }
            KeyCode::Char('q') => {
                if self.finished || !self.live {
                    return Ok(true);
                }
                self.quit_modal = true
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
                    .push_front("EXPORT // CSV + JSON COMPLETE".into())
            }
        }
        Ok(())
    }
}
fn push(q: &mut Vec<u64>, v: u64) {
    q.push(v);
    if q.len() > HISTORY {
        q.remove(0);
    }
}

pub fn render_update_text(
    update: MissionControlUpdate,
    width: u16,
    height: u16,
    page: usize,
) -> Result<String, ConsoleError> {
    use ratatui::backend::TestBackend;
    let config = ConsoleConfig {
        recording: None,
        sound: SoundProfile::Off,
        ..ConsoleConfig::default()
    };
    let mut app = App::new(config, false, None)?;
    app.page = page.min(6);
    app.accept(update);
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
        text.push('\n')
    }
    Ok(text)
}
fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
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
            .block(Block::default().borders(Borders::BOTTOM))
            .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan).bold())
            .divider(" │ "),
        root[1],
    );
    match app.page {
        0 => flight_page(frame, app, root[2]),
        1 => trajectory_page(frame, app, root[2]),
        2 => gnc_page(frame, app, root[2]),
        3 => nav_page(frame, app, root[2]),
        4 => vehicle_page(frame, app, root[2]),
        5 => network_page(frame, app, root[2]),
        _ => sim_page(frame, app, root[2]),
    }
    render_footer(frame, app, root[3]);
    if app.quit_modal {
        render_modal(frame, area)
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
    } else {
        "LIVE"
    };
    let (t, epoch, phase) = app
        .latest
        .map(|v| (time(&v), v.epoch, phase_name(v.director.phase)))
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
            "  {state}  T+{t:08.2}  EPOCH {epoch:05}  {phase}  PACE {pace}  {}  SND {}",
            app.config.units.label(),
            app.config.sound.label()
        )),
    ]);
    f.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        a,
    )
}
fn render_footer(f: &mut Frame, app: &App, a: Rect) {
    let rec = app
        .record_path
        .as_ref()
        .map(|p| format!("REC {}", p.display()))
        .unwrap_or_else(|| "REC OFF".into());
    let keys = if app.replay.is_some() {
        "←/→ SEEK  SPACE PLAY  HOME/END  E EXPORT  Q EXIT"
    } else {
        "F1–F7 CONSOLES  SPACE HOLD  . STEP  [/] RATE  U UNITS  S SOUND  B MARK  F FREEZE  E EXPORT  Q QUIT"
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(keys, Style::default().fg(Color::Cyan)),
            Span::raw("   "),
            Span::styled(rec, Style::default().fg(Color::DarkGray)),
        ]))
        .alignment(Alignment::Center),
        a,
    )
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
fn panel<'a>(title: &'a str) -> Block<'a> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title_style(Style::default().fg(Color::Cyan).bold())
}
fn metric(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<16}"), Style::default().fg(Color::DarkGray)),
        Span::styled(value, Style::default().fg(Color::White).bold()),
    ])
}
fn flight_page(f: &mut Frame, app: &App, a: Rect) {
    let c = split3(a);
    if let Some(v) = app.latest {
        let left = vec![
            metric("ALTITUDE", fmt_alt(altitude(&v), app.config.units)),
            metric("SPEED", fmt_speed(speed(&v), app.config.units)),
            metric("MACH", format!("{:.3}", mach(&v))),
            metric("DYN PRESSURE", format!("{:.2} kPa", q(&v))),
            metric("MISSION TIME", format!("{:.2} s", time(&v))),
            metric("DOWNRANGE", format!("{:.2} km", downrange(&v))),
        ];
        f.render_widget(Paragraph::new(left).block(panel("FLIGHT DYNAMICS")), c[0]);
        let center = vec![
            metric("PHASE", phase_name(v.director.phase).into()),
            metric(
                "ACTIVE STAGE",
                format!("S-{}", v.director.active_stage.saturating_add(1)),
            ),
            metric(
                "MASS",
                format!("{:.2} t", v.director.total_mass_q12 as f64 / 4096.0),
            ),
            metric(
                "PROPELLANT",
                format!("{:.2} t", v.director.active_propellant_q12 as f64 / 4096.0),
            ),
            metric(
                "GIMBAL P/Y",
                format!(
                    "{:+.3} / {:+.3}",
                    v.director.gimbal_applied_q16[0] as f64 / 65536.0,
                    v.director.gimbal_applied_q16[1] as f64 / 65536.0
                ),
            ),
            metric("EVENTS", event_names(v.director.events)),
        ];
        f.render_widget(Paragraph::new(center).block(panel("VEHICLE")), c[1]);
        let status = vec![
            metric(
                "FLIGHT COMPUTER",
                if v.status.map(|s| s.alarms).unwrap_or(0) == 0 {
                    "GO".into()
                } else {
                    "ALARM".into()
                },
            ),
            metric(
                "GROUND TRACK",
                if v.ground_estimate.is_some() {
                    "GO".into()
                } else {
                    "ACQUIRING".into()
                },
            ),
            metric(
                "LINK CELLS",
                format!("{} / {}", v.world_cells, v.flight_cells),
            ),
            metric("NAV ERROR", format!("{:.3} km", nav_error(&v))),
            metric(
                "DEADLINES",
                format!("{}", v.status.map(|s| s.deadline_misses).unwrap_or(0)),
            ),
            metric("CHECKSUM", format!("{:08X}", v.transcript_checksum)),
        ];
        f.render_widget(Paragraph::new(status).block(panel("FLIGHT DIRECTOR")), c[2])
    } else {
        f.render_widget(
            Paragraph::new("AWAITING KLR6 TELEMETRY")
                .alignment(Alignment::Center)
                .block(panel("FLIGHT DIRECTOR")),
            a,
        )
    }
}
fn trajectory_page(f: &mut Frame, app: &App, a: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(a);
    let top = split3(rows[0]);
    f.render_widget(
        Sparkline::default()
            .block(panel("ALTITUDE TREND"))
            .data(&app.alt)
            .style(Style::default().fg(Color::Cyan)),
        top[0],
    );
    f.render_widget(
        Sparkline::default()
            .block(panel("VELOCITY TREND"))
            .data(&app.speed)
            .style(Style::default().fg(Color::Green)),
        top[1],
    );
    f.render_widget(
        Sparkline::default()
            .block(panel("DYNAMIC PRESSURE"))
            .data(&app.q)
            .style(Style::default().fg(Color::Yellow)),
        top[2],
    );
    if let Some(v) = app.latest {
        let p = v.director.position_q12.map(|x| x as f64 / 4096.0);
        let vel = v.director.velocity_q24.map(|x| x as f64 / 16_777_216.0);
        let text = vec![
            metric(
                "ECEF POSITION",
                format!("[{:+.2}, {:+.2}, {:+.2}] km", p[0], p[1], p[2]),
            ),
            metric(
                "ECEF VELOCITY",
                format!("[{:+.4}, {:+.4}, {:+.4}] km/s", vel[0], vel[1], vel[2]),
            ),
            metric(
                "RADIUS",
                format!("{:.3} km", altitude(&v) + EARTH_RADIUS_KM),
            ),
            metric("GROUND RANGE", format!("{:.3} km", downrange(&v))),
            metric("FLIGHT PATH", format!("{:+.3} deg", flight_path(&v))),
        ];
        f.render_widget(
            Paragraph::new(text).block(panel("TRAJECTORY SOLUTION")),
            rows[1],
        )
    }
}
fn gnc_page(f: &mut Frame, app: &App, a: Rect) {
    let c = split3(a);
    if let Some(v) = app.latest {
        let angles = v
            .inertial
            .platform_angle
            .map(|x| x as f64 / 32768.0 * 180.0);
        let rates = v.director.angular_rate_q24.map(|x| x as f64 / 16_777_216.0);
        let g = vec![
            metric(
                "PLATFORM XYZ",
                format!("{:+.2} {:+.2} {:+.2}°", angles[0], angles[1], angles[2]),
            ),
            metric(
                "BODY RATES",
                format!("{:+.4} {:+.4} {:+.4}", rates[0], rates[1], rates[2]),
            ),
            metric("GUIDE START", format!("{:?}", v.guidance.start)),
            metric("GUIDE END", format!("{:?}", v.guidance.end)),
            metric("GUIDE RATE", format!("{:?}", v.guidance.rate)),
        ];
        f.render_widget(Paragraph::new(g).block(panel("GUIDANCE")), c[0]);
        let ctrl = vec![
            metric(
                "REQUEST P/Y",
                format!("{:?}", v.director.gimbal_requested_q16),
            ),
            metric("LAGGED P/Y", format!("{:?}", v.director.gimbal_lagged_q16)),
            metric(
                "APPLIED P/Y",
                format!("{:?}", v.director.gimbal_applied_q16),
            ),
            metric("RCS COMMAND", format!("{:?}", v.command.rcs)),
            metric(
                "AOA SINE",
                format!(
                    "{:+.5}",
                    v.director.angle_of_attack_sine_q16 as f64 / 65536.0
                ),
            ),
        ];
        f.render_widget(Paragraph::new(ctrl).block(panel("CONTROL")), c[1]);
        f.render_widget(
            Sparkline::default()
                .block(panel("NAV ERROR HISTORY"))
                .data(&app.nav_error)
                .style(Style::default().fg(Color::Magenta)),
            c[2],
        )
    }
}
fn nav_page(f: &mut Frame, app: &App, a: Rect) {
    let c = split3(a);
    if let Some(v) = app.latest {
        let onboard = v
            .status
            .map(|s| (s.navigation_position_q12, s.navigation_velocity_q24));
        let ground = v.ground_estimate.map(|s| (s.position_q12, s.velocity_q24));
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
            Paragraph::new(make(onboard)).block(panel("ONBOARD NAVIGATION")),
            c[0],
        );
        f.render_widget(
            Paragraph::new(make(ground)).block(panel("GROUND ESTIMATE")),
            c[1],
        );
        let cmp = v
            .comparison
            .map(|x| {
                vec![
                    metric("POSITION Δ", format!("{:?}", x.position_delta_q12)),
                    metric("VELOCITY Δ", format!("{:?}", x.velocity_delta_q24)),
                    metric("NORM", format!("{:.6} km", nav_error(&v))),
                ]
            })
            .unwrap_or_else(|| vec![metric("COMPARISON", "PENDING".into())]);
        f.render_widget(
            Paragraph::new(cmp).block(panel("INDEPENDENT CROSSCHECK")),
            c[2],
        )
    }
}
fn vehicle_page(f: &mut Frame, app: &App, a: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(a);
    if let Some(v) = app.latest {
        let fuel = (v.director.active_propellant_q12.max(0) as f64
            / (v.director.total_mass_q12.max(1) as f64))
            .clamp(0.0, 1.0);
        f.render_widget(
            Gauge::default()
                .block(panel("ACTIVE-STAGE PROPELLANT FRACTION"))
                .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
                .ratio(fuel)
                .label(format!("{:.1}%", fuel * 100.0)),
            Rect {
                x: rows[0].x + 2,
                y: rows[0].y + 2,
                width: rows[0].width.saturating_sub(4),
                height: 3,
            },
        );
        let c = split3(rows[1]);
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
            ])
            .block(panel("MASS PROPERTIES")),
            c[0],
        );
        f.render_widget(
            Paragraph::new(vec![
                metric(
                    "STAGE",
                    format!("S-{}", v.director.active_stage.saturating_add(1)),
                ),
                metric("PHASE", phase_name(v.director.phase).into()),
                metric("SUBSTEP", format!("{}", v.director.substep)),
                metric("EVENTS", event_names(v.director.events)),
            ])
            .block(panel("SEQUENCER")),
            c[1],
        );
        f.render_widget(
            Paragraph::new(vec![
                metric("FLEX MODES", format!("{:?}", v.director.flexible_q24)),
                metric("DYN PRESSURE", format!("{:.3} kPa", q(&v))),
                metric("MACH", format!("{:.3}", mach(&v))),
            ])
            .block(panel("LOADS & STRUCTURE")),
            c[2],
        )
    }
}
fn network_page(f: &mut Frame, app: &App, a: Rect) {
    let c = split3(a);
    if let Some(v) = app.latest {
        f.render_widget(
            Paragraph::new(vec![
                metric("WORLD CELLS", format!("{}", v.world_cells)),
                metric("FLIGHT CELLS", format!("{}", v.flight_cells)),
                metric("TRANSCRIPT", format!("{:08X}", v.transcript_checksum)),
                metric("MC ALARMS", format!("{:04X}", v.mission_control_alarms)),
            ])
            .block(panel("KLR6 NETWORK")),
            c[0],
        );
        f.render_widget(
            Paragraph::new(vec![
                metric(
                    "FIX ID",
                    v.ground_fix
                        .map(|x| x.fix_id.to_string())
                        .unwrap_or_else(|| "—".into()),
                ),
                metric(
                    "FIX COUNT",
                    v.ground_estimate
                        .map(|x| x.fixes.to_string())
                        .unwrap_or_else(|| "0".into()),
                ),
                metric(
                    "GROUND CRC",
                    v.ground_estimate
                        .map(|x| format!("{:08X}", x.checksum))
                        .unwrap_or_else(|| "—".into()),
                ),
                metric(
                    "VALIDITY",
                    v.ground_fix
                        .map(|x| format!("{:04X}", x.validity))
                        .unwrap_or_else(|| "—".into()),
                ),
            ])
            .block(panel("TRACKING NETWORK")),
            c[1],
        );
        let items = app
            .events
            .iter()
            .take(10)
            .map(|x| ListItem::new(x.as_str()))
            .collect::<Vec<_>>();
        f.render_widget(List::new(items).block(panel("EVENT LOG")), c[2])
    }
}
fn sim_page(f: &mut Frame, app: &App, a: Rect) {
    let c = split3(a);
    if let Some(v) = app.latest {
        f.render_widget(
            Paragraph::new(vec![
                metric("TRUE POSITION", format!("{:?}", v.director.position_q12)),
                metric("TRUE VELOCITY", format!("{:?}", v.director.velocity_q24)),
                metric("TRUE ACCEL", format!("{:?}", v.director.acceleration_q28)),
                metric("ATTITUDE Q30", format!("{:?}", v.director.attitude_q30)),
            ])
            .block(panel("SIM DIRECTOR // OMNISCIENT")),
            c[0],
        );
        f.render_widget(
            Paragraph::new(vec![
                metric("IMU VALID", format!("{:02X}", v.inertial.validity)),
                metric(
                    "AID VALID",
                    v.aid
                        .map(|x| format!("{:04X}", x.validity))
                        .unwrap_or_else(|| "—".into()),
                ),
                metric("COMMAND", format!("{:?}", v.command.gimbal)),
                metric(
                    "STATUS MODE",
                    v.status
                        .map(|x| x.mode.to_string())
                        .unwrap_or_else(|| "—".into()),
                ),
                metric(
                    "FLIGHT CRC",
                    v.status
                        .map(|x| format!("{:08X}", x.flight_checksum))
                        .unwrap_or_else(|| "—".into()),
                ),
            ])
            .block(panel("INJECTED / OBSERVED")),
            c[1],
        );
        f.render_widget(
            Paragraph::new(vec![
                metric("BOOKMARKS", format!("{}", app.bookmarks.len())),
                metric("FREEZE", format!("{}", app.freeze)),
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
            ])
            .block(panel("SIM OPERATIONS")),
            c[2],
        )
    }
}
fn render_modal(f: &mut Frame, a: Rect) {
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
        .block(
            Block::default()
                .title(" FLIGHT DIRECTOR CONFIRMATION ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title_style(Style::default().fg(Color::Red).bold()),
        ),
        r,
    )
}

fn time(v: &MissionControlUpdate) -> f64 {
    v.director.time_q16 as f64 / 65536.0
}
fn q(v: &MissionControlUpdate) -> f64 {
    v.director.dynamic_pressure_q16 as f64 / 65536.0
}
fn mach(v: &MissionControlUpdate) -> f64 {
    v.director.mach_q16 as f64 / 65536.0
}
fn altitude(v: &MissionControlUpdate) -> f64 {
    let p = v.director.position_q12;
    ((p.iter().map(|x| (*x as f64 / 4096.0).powi(2)).sum::<f64>()).sqrt() - EARTH_RADIUS_KM)
        .max(-EARTH_RADIUS_KM)
}
fn speed(v: &MissionControlUpdate) -> f64 {
    (v.director
        .velocity_q24
        .iter()
        .map(|x| (*x as f64 / 16_777_216.0).powi(2))
        .sum::<f64>())
    .sqrt()
}
fn downrange(v: &MissionControlUpdate) -> f64 {
    let p = v.director.position_q12.map(|x| x as f64 / 4096.0);
    p[1].atan2(p[0]).abs() * EARTH_RADIUS_KM
}
fn flight_path(v: &MissionControlUpdate) -> f64 {
    let p = v.director.position_q12.map(|x| x as f64 / 4096.0);
    let vel = v.director.velocity_q24.map(|x| x as f64 / 16_777_216.0);
    let dot = p.iter().zip(vel).map(|(a, b)| a * b).sum::<f64>();
    let pn = p.iter().map(|x| x * x).sum::<f64>().sqrt();
    (dot / (pn * speed(v)).max(1e-9))
        .clamp(-1.0, 1.0)
        .asin()
        .to_degrees()
}
fn nav_error(v: &MissionControlUpdate) -> f64 {
    v.comparison
        .map(|c| {
            (c.position_delta_q12
                .iter()
                .map(|x| (*x as f64 / 4096.0).powi(2))
                .sum::<f64>())
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
        (8, "IMPACT"),
        (16, "END"),
        (32, "GPS ACQ"),
        (64, "GPS LOST"),
        (128, "ABORT"),
        (256, "RCS DEPLETED"),
        (512, "GIMBAL JAM"),
        (1024, "STAR ACQ"),
        (2048, "STAR LOST"),
    ];
    let mut found = Vec::new();
    for (mask, name) in names {
        if e & mask != 0 {
            found.push(name)
        }
    }
    if found.is_empty() {
        format!("0x{e:04X}")
    } else {
        found.join(" + ")
    }
}
fn fmt_alt(km: f64, u: UnitSystem) -> String {
    match u {
        UnitSystem::Si => format!("{km:.3} km"),
        UnitSystem::Us => format!("{:.3} mi", km * 0.621371),
        UnitSystem::Dual => format!("{km:.3} km / {:.2} mi", km * 0.621371),
    }
}
fn fmt_speed(kms: f64, u: UnitSystem) -> String {
    match u {
        UnitSystem::Si => format!("{kms:.4} km/s"),
        UnitSystem::Us => format!("{:.1} mph", kms * 2236.936),
        UnitSystem::Dual => format!("{kms:.4} km/s / {:.0} mph", kms * 2236.936),
    }
}
