//! Passive seven-page Mission Control for Phase 9.5 advanced-effector missions.
use crate::phase9_5_link::{Kmr9Recording, Phase95Sink, Phase95SplitEvidence, Phase95Update};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline, Tabs, Wrap};
use ratatui::{Frame, Terminal};
use std::collections::VecDeque;
use std::io::{self, Stderr};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

const PAGE_NAMES: [&str; 7] = [
    "F1 FLIGHT",
    "F2 TRAJECTORY",
    "F3 CONTROL",
    "F4 NAV",
    "F5 VEHICLE",
    "F6 LINK",
    "F7 TRUTH",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvancedConsolePace {
    Fast,
    Realtime,
}

#[derive(Clone, Debug)]
pub struct AdvancedConsoleConfig {
    pub title: String,
    pub pace: AdvancedConsolePace,
    pub auto_exit: bool,
}
impl Default for AdvancedConsoleConfig {
    fn default() -> Self {
        Self {
            title: "KSA64 // ADVANCED-EFFECTOR MISSION CONTROL".into(),
            pace: AdvancedConsolePace::Realtime,
            auto_exit: false,
        }
    }
}
#[derive(Debug)]
pub enum AdvancedConsoleError {
    Io(io::Error),
    Worker,
}
impl From<io::Error> for AdvancedConsoleError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
#[derive(Clone, Debug)]
pub struct AdvancedConsoleRun {
    pub evidence: Phase95SplitEvidence,
    pub updates: Vec<Phase95Update>,
}
impl AdvancedConsoleRun {
    pub fn recording(&self) -> Kmr9Recording {
        Kmr9Recording {
            schema: "ksa64.kmr9-v1".into(),
            placement: self.evidence.placement,
            releases: self.evidence.releases,
            terminal_checksums: self.updates.last().map_or([0; 8], |u| u.checksums),
            updates: self.updates.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AdvancedConsoleModel {
    pub page: usize,
    pub latest: Option<Phase95Update>,
    pub history: VecDeque<Phase95Update>,
    pub recording: Vec<Phase95Update>,
    pub planned: Vec<[f64; 3]>,
    pub events: VecDeque<String>,
    pub complete: bool,
    pub evidence: Option<Phase95SplitEvidence>,
}
impl AdvancedConsoleModel {
    pub fn new() -> Self {
        let planned = crate::phase8::run_checked_in_phase8()
            .map(|run| {
                run.trace
                    .into_iter()
                    .map(|point| point.position_m)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            page: 0,
            latest: None,
            history: VecDeque::new(),
            recording: Vec::new(),
            planned,
            events: VecDeque::new(),
            complete: false,
            evidence: None,
        }
    }
    pub fn accept(&mut self, update: Phase95Update) {
        if update.events != 0 {
            self.events.push_back(format!(
                "T+{:7.2}  WORLD EVENTS {:04X}",
                update.time_s, update.events
            ));
        }
        if update.authority_state & 8 != 0 {
            self.events.push_back(format!(
                "T+{:7.2}  AUTHORITY HANDOFF {:02X}",
                update.time_s, update.authority_state
            ));
        }
        if update.command_discrete != 0 {
            self.events.push_back(format!(
                "T+{:7.2}  DISCRETE COMMAND {:02X}",
                update.time_s, update.command_discrete
            ));
        }
        while self.events.len() > 12 {
            self.events.pop_front();
        }
        self.recording.push(update.clone());
        self.history.push_back(update.clone());
        while self.history.len() > 4096 {
            self.history.pop_front();
        }
        self.latest = Some(update);
    }
}
impl Default for AdvancedConsoleModel {
    fn default() -> Self {
        Self::new()
    }
}

fn block(title: &str) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
}
fn fmt3(v: [f64; 3], unit: &str) -> String {
    format!("E {:+9.2}  N {:+9.2}  U {:+9.2} {unit}", v[0], v[1], v[2])
}
fn fmt_torque(v: [f64; 3]) -> String {
    format!("R {:+8.4}  P {:+8.4}  Y {:+8.4} N m", v[0], v[1], v[2])
}
fn turn16_deg(value: i16) -> f64 {
    f64::from(value) * 360.0 / 65_536.0
}
fn phase_name(phase: u8) -> &'static str {
    match phase {
        0 => "RAIL",
        1 => "POWERED",
        2 => "COAST",
        3 => "DROGUE",
        4 => "MAIN",
        5 => "COMPLETE",
        _ => "FAILED",
    }
}
fn air_name(source: u8) -> &'static str {
    match source {
        1 => "PITOT",
        2 => "CONSERVATIVE FALLBACK",
        _ => "UNAVAILABLE",
    }
}
fn authority_name(bits: u16) -> String {
    let mut names = Vec::new();
    if bits & 1 != 0 {
        names.push("GIMBAL");
    }
    if bits & 2 != 0 {
        names.push("CANARD");
    }
    if bits & 4 != 0 {
        names.push("RCS");
    }
    if bits & 8 != 0 {
        names.push("HANDOFF");
    }
    if names.is_empty() {
        "NONE".into()
    } else {
        names.join(" + ")
    }
}
fn header(frame: &mut Frame<'_>, area: Rect, model: &AdvancedConsoleModel, title: &str) -> Rect {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    let tabs = Tabs::new(
        PAGE_NAMES
            .iter()
            .map(|name| Line::from(*name))
            .collect::<Vec<_>>(),
    )
    .select(model.page)
    .block(Block::default().title(title).borders(Borders::ALL))
    .highlight_style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(tabs, rows[0]);
    let status = if model.complete {
        "COMPLETE  F1-F7 or arrows navigate  Q/Esc closes presentation"
    } else {
        "LIVE EXTERNALLY PACED  F1-F7 or arrows navigate  world waits for each flight release"
    };
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(if model.complete {
            Color::Green
        } else {
            Color::Yellow
        })),
        rows[2],
    );
    rows[1]
}
fn plot(points: &[(f64, f64, char)], width: usize, height: usize) -> String {
    if width < 3 || height < 3 || points.is_empty() {
        return "waiting for trajectory...".into();
    }
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for (x, y, _) in points {
        min_x = min_x.min(*x);
        max_x = max_x.max(*x);
        min_y = min_y.min(*y);
        max_y = max_y.max(*y);
    }
    if (max_x - min_x).abs() < 1e-9 {
        max_x = min_x + 1.0;
    }
    if (max_y - min_y).abs() < 1e-9 {
        max_y = min_y + 1.0;
    }
    let mut grid = vec![vec![' '; width]; height];
    for (x, y, c) in points {
        let px = (((x - min_x) / (max_x - min_x)) * (width - 1) as f64).round() as usize;
        let py =
            height - 1 - (((y - min_y) / (max_y - min_y)) * (height - 1) as f64).round() as usize;
        grid[py.min(height - 1)][px.min(width - 1)] = *c;
    }
    grid.into_iter()
        .map(|row| row.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
fn page_flight(frame: &mut Frame<'_>, area: Rect, model: &AdvancedConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        frame.render_widget(
            Paragraph::new("AWAITING SENSOR EPOCH").block(block("FLIGHT DIRECTOR")),
            area,
        );
        return;
    };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(4)])
        .split(cols[0]);
    let text = vec![
        Line::from(vec![
            Span::styled(
                format!("T+{:8.3} s", v.time_s),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("   EPOCH {:5}   {}", v.epoch, phase_name(v.phase))),
        ]),
        Line::from(fmt3(v.onboard_position_m, "m")),
        Line::from(fmt3(v.onboard_velocity_mps, "m/s")),
        Line::from(format!(
            "MACH {:6.3}   Q {:9.1} Pa   AIR {}",
            v.mach,
            v.dynamic_pressure_pa,
            air_name(v.air_data_source)
        )),
        Line::from(format!(
            "AUTHORITY {:20}  SAT {}",
            authority_name(v.authority_state),
            v.saturation_count
        )),
        Line::from(format!(
            "ARMED {:5}  DROGUE {:5}  MAIN {:5}  SAFE {:5}",
            v.armed, v.drogue_latched, v.main_latched, v.safe
        )),
        Line::from(format!(
            "ALARMS {:04X}  DEADLINES {}  EVENTS {:04X}",
            v.alarms, v.deadline_misses, v.events
        )),
    ];
    frame.render_widget(
        Paragraph::new(text).block(block("FLIGHT DIRECTOR / PUBLIC TELEMETRY")),
        rows[0],
    );
    let values = model
        .history
        .iter()
        .rev()
        .take(220)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|u| u.onboard_position_m[2].max(0.0) as u64)
        .collect::<Vec<_>>();
    frame.render_widget(
        Sparkline::default()
            .block(block("ONBOARD ALTITUDE"))
            .data(&values)
            .style(Style::default().fg(Color::Cyan)),
        rows[1],
    );
    let events = model
        .events
        .iter()
        .rev()
        .map(|e| Line::from(e.clone()))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(events)
            .block(block("EVENTS / HANDOFFS"))
            .wrap(Wrap { trim: true }),
        cols[1],
    );
}
fn page_trajectory(frame: &mut Frame<'_>, area: Rect, model: &AdvancedConsoleModel) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);
    let mut side = Vec::new();
    let mut top = Vec::new();
    for p in model.planned.iter().step_by(4) {
        side.push(((p[0] * p[0] + p[1] * p[1]).sqrt(), p[2], '.'));
        top.push((p[0], p[1], '.'));
    }
    for p in &model.history {
        let g = p.ground_position_m;
        let n = p.onboard_position_m;
        let t = p.truth_position_m;
        side.push(((g[0] * g[0] + g[1] * g[1]).sqrt(), g[2], 'G'));
        side.push(((n[0] * n[0] + n[1] * n[1]).sqrt(), n[2], 'N'));
        side.push(((t[0] * t[0] + t[1] * t[1]).sqrt(), t[2], 'T'));
        top.push((g[0], g[1], 'G'));
        top.push((n[0], n[1], 'N'));
        top.push((t[0], t[1], 'T'));
    }
    frame.render_widget(
        Paragraph::new(plot(
            &side,
            cols[0].width.saturating_sub(2) as usize,
            cols[0].height.saturating_sub(2) as usize,
        ))
        .block(block("SIDE  . PLAN  G GROUND  N ONBOARD  T PHYSICAL")),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(plot(
            &top,
            cols[1].width.saturating_sub(2) as usize,
            cols[1].height.saturating_sub(2) as usize,
        ))
        .block(block("TOP-DOWN / DRIFT")),
        cols[1],
    );
}
fn page_control(frame: &mut Frame<'_>, area: Rect, model: &AdvancedConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12),
            Constraint::Length(3),
            Constraint::Min(4),
        ])
        .split(area);
    let pulse_total: u32 = v.rcs_pulse_quanta.iter().map(|q| u32::from(*q)).sum();
    let text=format!("REQUESTED  {}\nACHIEVED   {}\nRESIDUAL   {}\n\nGIMBAL CMD/APPLIED  [{:+6.2}, {:+6.2}] / [{:+6.2}, {:+6.2}] deg\nCANARD CMD deg       {:?}\nCANARD APPLIED deg   {:?}\nRCS PULSES {} quanta  VALVES {:03X}  AUTHORITY {}",
        fmt_torque(v.requested_torque_nm),fmt_torque(v.achieved_torque_nm),fmt_torque(v.residual_torque_nm),
        turn16_deg(v.commanded_gimbal[0]),turn16_deg(v.commanded_gimbal[1]),turn16_deg(v.applied_gimbal[0]),turn16_deg(v.applied_gimbal[1]),
        v.commanded_canards.map(|x|(turn16_deg(x)*100.0).round()/100.0),v.applied_canards.map(|x|(turn16_deg(x)*100.0).round()/100.0),pulse_total,v.valve_open_mask,authority_name(v.authority_state));
    frame.render_widget(
        Paragraph::new(text).block(block("GUIDANCE / PRIORITY-RESIDUAL ALLOCATION")),
        rows[0],
    );
    let requested = v.requested_torque_nm.iter().map(|x| x.abs()).sum::<f64>();
    let residual = v.residual_torque_nm.iter().map(|x| x.abs()).sum::<f64>();
    let ratio = if requested > 1e-9 {
        (1.0 - residual / requested).clamp(0.0, 1.0)
    } else {
        1.0
    };
    frame.render_widget(
        Gauge::default()
            .block(block("DEMAND SATISFIED"))
            .ratio(ratio)
            .gauge_style(Style::default().fg(if ratio < 0.9 {
                Color::Red
            } else {
                Color::Green
            })),
        rows[1],
    );
    frame.render_widget(Paragraph::new("Demand is body roll/pitch/yaw torque. Installed effectors consume bounded authority in pack order; exact residual passes to the next family. Pulse commands are one-shot and never replayed.").block(block("CONTROL CONTRACT")).wrap(Wrap{trim:true}),rows[2]);
}
fn page_nav(frame: &mut Frame<'_>, area: Rect, model: &AdvancedConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        return;
    };
    let pr = [
        v.onboard_position_m[0] - v.ground_position_m[0],
        v.onboard_position_m[1] - v.ground_position_m[1],
        v.onboard_position_m[2] - v.ground_position_m[2],
    ];
    let vr = [
        v.onboard_velocity_mps[0] - v.ground_velocity_mps[0],
        v.onboard_velocity_mps[1] - v.ground_velocity_mps[1],
        v.onboard_velocity_mps[2] - v.ground_velocity_mps[2],
    ];
    let text=format!("ONBOARD POSITION  {}\nGROUND ESTIMATE   {}\nPOSITION RESIDUAL {}\n\nONBOARD VELOCITY  {}\nGROUND ESTIMATE   {}\nVELOCITY RESIDUAL {}\n\nAIR DATA {:22}  SENSOR VALID {:04X}  AID VALID {:04X}\nWIND ENVELOPE {}\nNAV CHECKSUM {:08X}",fmt3(v.onboard_position_m,"m"),fmt3(v.ground_position_m,"m"),fmt3(pr,"m"),fmt3(v.onboard_velocity_mps,"m/s"),fmt3(v.ground_velocity_mps,"m/s"),fmt3(vr,"m/s"),air_name(v.air_data_source),v.sensor_validity,v.aid_validity,fmt3(v.wind_mps,"m/s"),v.checksums[2]);
    frame.render_widget(
        Paragraph::new(text).block(block("TRUTH-BLIND NAVIGATION / AIR DATA")),
        area,
    );
}
fn page_vehicle(frame: &mut Frame<'_>, area: Rect, model: &AdvancedConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        return;
    };
    let text=format!("FIRESTORM-M9 RESEARCH DERIVATIVE\nMASS {:8.3} kg   CG {:7.3} m from nose   STATIC MARGIN {:6.3} cal\nMACH {:7.3}      Q {:9.1} Pa             AOA {:7.3} deg\nWIND {}\n\nCANARD HINGE N m  [{:+7.4}, {:+7.4}, {:+7.4}, {:+7.4}]\nRCS PROPELLANT {:7.4} kg   SUPPLY SCALE {:6.3}   PRESSURE {:9.1} Pa\nRCS FORCE  {}\nRCS TORQUE {}\n\nRAIL -> GIMBAL/CANARD/RCS AUTHORITY -> COAST HANDOFF -> RECOVERY SAFE",
        v.mass_kg,v.cg_from_nose_m,v.static_margin,v.mach,v.dynamic_pressure_pa,v.angle_of_attack_deg,fmt3(v.wind_mps,"m/s"),v.hinge_moment_nm[0],v.hinge_moment_nm[1],v.hinge_moment_nm[2],v.hinge_moment_nm[3],v.propellant_kg,v.supply_scale,v.supply_pressure_pa,fmt3(v.rcs_force_body_n,"N"),fmt_torque(v.rcs_torque_body_nm));
    frame.render_widget(
        Paragraph::new(text).block(block("VEHICLE / EFFECTORS / CONSUMABLES")),
        area,
    );
}
fn page_link(frame: &mut Frame<'_>, area: Rect, model: &AdvancedConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        return;
    };
    let e = model.evidence;
    let text=format!("KLF6 OUTER / KLR9 RAW CELLS / EXACT 32 HZ SIMULATED CLOCK\nMEASUREMENT EPOCH {:5}   COMMAND EFFECTIVE EPOCH {:5}\nCOMMAND FLAGS {:02X}   DISCRETE {:02X}   VALVE EDGES {}\nDEADLINE MISSES {}   ALARMS {:04X}\n\nCHECKSUM CHAINS\n TRUTH {:08X}  SENSOR {:08X}  NAV {:08X}  DEMAND {:08X}\n COMMAND {:08X}  ALLOC {:08X}  STATUS {:08X}  FLIGHT CMD {:08X}\n\nPLACEMENT\n host world + externally paced stock C64 flight: ACCEPTED BASELINE\n simulated time waits for each response; this is not a realtime claim\n\nFINAL {}",v.epoch,v.epoch.wrapping_add(1),v.command_flags,v.command_discrete,v.valve_edge_count,v.deadline_misses,v.alarms,v.checksums[0],v.checksums[1],v.checksums[2],v.checksums[3],v.checksums[4],v.checksums[5],v.checksums[6],v.checksums[7],e.map(|x|format!("{:?} / {} releases",x.placement,x.releases)).unwrap_or_else(||"IN PROGRESS".into()));
    frame.render_widget(
        Paragraph::new(text).block(block("LINK / EPOCHS / EXACTNESS")),
        area,
    );
}
fn page_truth(frame: &mut Frame<'_>, area: Rect, model: &AdvancedConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        return;
    };
    let text=format!("SIMULATION TRUTH - F7 ONLY; NOT AVAILABLE TO FLIGHT SOFTWARE\n\nPOSITION {}\nVELOCITY {}\nATTITUDE q [{:+.6}, {:+.6}, {:+.6}, {:+.6}]\nANGULAR RATE {}\nPHASE {}  EVENTS {:04X}\nMASS {:.4} kg  MACH {:.4}  Q {:.1} Pa  AOA {:.4} deg\nRCS FORCE {}\nRCS TORQUE {}\nTRUTH CHAIN {:08X}\n\nF7 is engineering comparison evidence, not launch approval, certification, regulatory evidence, or safety authority.",fmt3(v.truth_position_m,"m"),fmt3(v.truth_velocity_mps,"m/s"),v.attitude[0],v.attitude[1],v.attitude[2],v.attitude[3],fmt3(v.angular_rate_rad_s,"rad/s"),phase_name(v.phase),v.events,v.mass_kg,v.mach,v.dynamic_pressure_pa,v.angle_of_attack_deg,fmt3(v.rcs_force_body_n,"N"),fmt_torque(v.rcs_torque_body_nm),v.checksums[0]);
    frame.render_widget(
        Paragraph::new(text).block(block("SIMULATION TRUTH / INJECTED WORLD")),
        area,
    );
}
pub fn render_advanced_console(frame: &mut Frame<'_>, model: &AdvancedConsoleModel, title: &str) {
    let area = header(frame, frame.area(), model, title);
    match model.page {
        0 => page_flight(frame, area, model),
        1 => page_trajectory(frame, area, model),
        2 => page_control(frame, area, model),
        3 => page_nav(frame, area, model),
        4 => page_vehicle(frame, area, model),
        5 => page_link(frame, area, model),
        _ => page_truth(frame, area, model),
    }
}

enum LiveEvent {
    Update(Box<Phase95Update>),
    Finish(Phase95SplitEvidence),
    Failed,
}
struct ChannelSink {
    tx: Sender<LiveEvent>,
    pace: AdvancedConsolePace,
}
impl Phase95Sink for ChannelSink {
    fn publish(&mut self, u: &Phase95Update) {
        let _ = self.tx.send(LiveEvent::Update(Box::new(u.clone())));
        if self.pace == AdvancedConsolePace::Realtime {
            thread::sleep(Duration::from_micros(31_250));
        }
    }
    fn finish(&mut self, e: &Phase95SplitEvidence) {
        let _ = self.tx.send(LiveEvent::Finish(*e));
    }
}
fn restore(terminal: &mut Terminal<CrosstermBackend<Stderr>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}
pub fn run_advanced_console_with_worker<F>(
    config: AdvancedConsoleConfig,
    worker: F,
) -> Result<AdvancedConsoleRun, AdvancedConsoleError>
where
    F: FnOnce(&mut dyn Phase95Sink) -> Result<(), ()> + Send + 'static,
{
    let (tx, rx): (Sender<LiveEvent>, Receiver<LiveEvent>) = mpsc::channel();
    let pace = config.pace;
    thread::spawn(move || {
        let mut sink = ChannelSink {
            tx: tx.clone(),
            pace,
        };
        if worker(&mut sink).is_err() {
            let _ = tx.send(LiveEvent::Failed);
        }
    });
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;
    let mut model = AdvancedConsoleModel::new();
    let mut answer = None;
    let result = 'console: loop {
        loop {
            match rx.try_recv() {
                Ok(LiveEvent::Update(u)) => model.accept(*u),
                Ok(LiveEvent::Finish(e)) => {
                    model.complete = true;
                    model.evidence = Some(e);
                    answer = Some(e);
                }
                Ok(LiveEvent::Failed) | Err(TryRecvError::Disconnected) => {
                    break 'console Err(AdvancedConsoleError::Worker)
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        terminal.draw(|f| render_advanced_console(f, &model, &config.title))?;
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::F(n) if (1..=7).contains(&n) => model.page = (n - 1) as usize,
                        KeyCode::Left => model.page = (model.page + 6) % 7,
                        KeyCode::Right => model.page = (model.page + 1) % 7,
                        KeyCode::Char('q') | KeyCode::Esc if answer.is_some() => {
                            break 'console Ok(AdvancedConsoleRun {
                                evidence: answer.unwrap(),
                                updates: model.recording,
                            })
                        }
                        _ => {}
                    }
                }
            }
        }
        if model.complete && config.auto_exit {
            break 'console Ok(AdvancedConsoleRun {
                evidence: answer.unwrap(),
                updates: model.recording,
            });
        }
    };
    restore(&mut terminal)?;
    result
}

pub fn recording_from_updates(
    updates: Vec<Phase95Update>,
    evidence: Phase95SplitEvidence,
) -> Kmr9Recording {
    Kmr9Recording {
        schema: "ksa64.kmr9-v1".into(),
        placement: evidence.placement,
        releases: evidence.releases,
        terminal_checksums: updates.last().map_or([0; 8], |u| u.checksums),
        updates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase9_5_link::{
        run_host_external_with_limit_observed, run_native_flight_endpoint, Phase95Placement,
    };
    use ratatui::backend::TestBackend;
    use std::net::{TcpListener, TcpStream};
    struct VecSink(Vec<Phase95Update>);
    impl Phase95Sink for VecSink {
        fn publish(&mut self, u: &Phase95Update) {
            self.0.push(u.clone());
        }
    }
    fn sample_model() -> AdvancedConsoleModel {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let endpoint = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            run_native_flight_endpoint(&mut stream).unwrap();
        });
        let mut stream = TcpStream::connect(address).unwrap();
        let mut sink = VecSink(Vec::new());
        let evidence =
            run_host_external_with_limit_observed(&mut stream, 64, Some(&mut sink)).unwrap();
        endpoint.join().unwrap();
        let mut model = AdvancedConsoleModel::new();
        for update in sink.0 {
            model.accept(update);
        }
        model.complete = true;
        model.evidence = Some(evidence);
        model
    }
    #[test]
    fn every_page_renders_at_full_and_compact_sizes() {
        let mut model = sample_model();
        for size in [(140, 45), (80, 24)] {
            for (page, name) in PAGE_NAMES.iter().enumerate() {
                model.page = page;
                let backend = TestBackend::new(size.0, size.1);
                let mut terminal = Terminal::new(backend).unwrap();
                terminal
                    .draw(|f| render_advanced_console(f, &model, "KSA64 TEST"))
                    .unwrap();
                let content = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|c| c.symbol())
                    .collect::<String>();
                assert!(content.contains(name.split_whitespace().last().unwrap()));
            }
        }
    }
    #[test]
    fn recording_round_trips_without_changing_evidence() {
        let model = sample_model();
        let evidence = model.evidence.unwrap();
        let recording = recording_from_updates(model.recording, evidence);
        let bytes = serde_json::to_vec(&recording).unwrap();
        let decoded: Kmr9Recording = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.releases, evidence.releases);
        assert_eq!(decoded.terminal_checksums, recording.terminal_checksums);
        assert_eq!(decoded.updates.len(), evidence.releases as usize);
        assert_eq!(decoded.placement, Phase95Placement::HostExternalFlight);
    }
}
