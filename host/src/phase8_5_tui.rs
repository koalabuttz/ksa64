//! Full seven-page Phase 8.5 Mission Control presentation.
use crate::phase8_5::{
    checked_in_reference, run_host_host, Kmr8Recording, Phase85RunEvidence, Phase85Sink,
    Phase85Update,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ksa64_interface::phase8_5::Kat8Frame;
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
    "F3 GNC",
    "F4 NAV",
    "F5 VEHICLE",
    "F6 LINK",
    "F7 TRUTH",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsolePace {
    Fast,
    Realtime,
}
#[derive(Clone, Debug)]
pub struct LocalConsoleConfig {
    pub gimbal: bool,
    pub pace: ConsolePace,
    pub title: String,
}
impl Default for LocalConsoleConfig {
    fn default() -> Self {
        Self {
            gimbal: false,
            pace: ConsolePace::Realtime,
            title: "KSA64 // LOCAL-ENU MISSION CONTROL".into(),
        }
    }
}
#[derive(Debug)]
pub enum LocalConsoleError {
    Io(io::Error),
    Worker,
    Mission,
}
impl From<io::Error> for LocalConsoleError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
#[derive(Clone, Debug)]
pub struct LocalConsoleModel {
    pub page: usize,
    pub latest: Option<Phase85Update>,
    pub history: VecDeque<Phase85Update>,
    pub planned: Vec<[f64; 3]>,
    pub events: VecDeque<String>,
    pub complete: bool,
    pub evidence: Option<Phase85RunEvidence>,
}
impl LocalConsoleModel {
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
            planned,
            events: VecDeque::new(),
            complete: false,
            evidence: None,
        }
    }
    pub fn accept(&mut self, update: Phase85Update) {
        if update.events != 0 {
            self.events.push_back(format!(
                "T+{:7.2}  EVENTS {:04X}",
                update.time_s, update.events
            ));
            while self.events.len() > 10 {
                self.events.pop_front();
            }
        }
        self.history.push_back(update.clone());
        while self.history.len() > 4096 {
            self.history.pop_front();
        }
        self.latest = Some(update);
    }
}
impl Default for LocalConsoleModel {
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
fn header(frame: &mut Frame<'_>, area: Rect, model: &LocalConsoleModel, title: &str) -> Rect {
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
        "COMPLETE  F1-F7 pages  ←/→ navigate  Q quit"
    } else {
        "LIVE  F1-F7 pages  ←/→ navigate  Q stop presentation"
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
        return "waiting for trajectory…".into();
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
        .join(
            "
",
        )
}
fn page_flight(frame: &mut Frame<'_>, area: Rect, model: &LocalConsoleModel) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(5)])
        .split(cols[0]);
    let Some(v) = model.latest.as_ref() else {
        frame.render_widget(
            Paragraph::new("AWAITING LIFTOFF").block(block("FLIGHT DIRECTOR")),
            area,
        );
        return;
    };
    let text = vec![
        Line::from(vec![
            Span::styled(
                format!("T+{:8.3} s", v.time_s),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("    PHASE {}    MODE {}", v.phase, v.flight_mode)),
        ]),
        Line::from(fmt3(v.onboard_position_m, "m")),
        Line::from(fmt3(v.onboard_velocity_mps, "m/s")),
        Line::from(format!(
            "MACH {:6.3}   Q {:9.1} Pa   THRUST {:8.1} N",
            v.mach, v.dynamic_pressure_pa, v.thrust_n
        )),
        Line::from(format!(
            "ARMED {:5}   DROGUE {:5}   MAIN {:5}   ALARMS {:04X}",
            v.armed, v.drogue_latched, v.main_latched, v.alarms
        )),
    ];
    frame.render_widget(
        Paragraph::new(text).block(block("FLIGHT DIRECTOR / PUBLIC TELEMETRY")),
        left[0],
    );
    let values: Vec<u64> = model
        .history
        .iter()
        .rev()
        .take(180)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|x| x.onboard_position_m[2].max(0.0) as u64)
        .collect();
    frame.render_widget(
        Sparkline::default()
            .block(block("ONBOARD ALTITUDE HISTORY"))
            .data(&values)
            .style(Style::default().fg(Color::Cyan)),
        left[1],
    );
    let events = model
        .events
        .iter()
        .rev()
        .map(|e| Line::from(e.clone()))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(events)
            .block(block("MISSION EVENTS"))
            .wrap(Wrap { trim: true }),
        cols[1],
    );
}
fn page_trajectory(frame: &mut Frame<'_>, area: Rect, model: &LocalConsoleModel) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);
    let mut side = Vec::new();
    let mut top = Vec::new();
    for p in model.planned.iter().step_by(4) {
        side.push(((p[0] * p[0] + p[1] * p[1]).sqrt(), p[2], '·'));
        top.push((p[0], p[1], '·'));
    }
    for p in &model.history {
        let g = p.ground_position_m;
        let o = p.onboard_position_m;
        side.push(((g[0] * g[0] + g[1] * g[1]).sqrt(), g[2], '●'));
        side.push((((o[0] * o[0] + o[1] * o[1]).sqrt()), o[2], '×'));
        top.push((g[0], g[1], '●'));
        top.push((o[0], o[1], '×'));
    }
    frame.render_widget(
        Paragraph::new(plot(
            &side,
            cols[0].width.saturating_sub(2) as usize,
            cols[0].height.saturating_sub(2) as usize,
        ))
        .block(block("SIDE PROFILE  · PLAN  ● GROUND  × ONBOARD")),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(plot(
            &top,
            cols[1].width.saturating_sub(2) as usize,
            cols[1].height.saturating_sub(2) as usize,
        ))
        .block(block("TOP-DOWN / RECOVERY DRIFT")),
        cols[1],
    );
}
fn page_gnc(frame: &mut Frame<'_>, area: Rect, model: &LocalConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(3),
            Constraint::Min(4),
        ])
        .split(area);
    let text = format!(
        "ATTITUDE VECTOR  {:?}
ANGULAR RATE     {:?}
TORQUE DEMAND    {:?}
GIMBAL COMMAND   {:?}
GIMBAL APPLIED   {:?}
AUTHORITY        {}",
        v.attitude_vector,
        v.angular_rate,
        v.control_demand,
        v.commanded_gimbal,
        v.applied_gimbal,
        if v.thrust_n > 0.0 { "POWERED" } else { "NONE" }
    );
    frame.render_widget(
        Paragraph::new(text).block(block("GUIDANCE / CONTROL / ALLOCATION")),
        rows[0],
    );
    let sat = (f64::from(v.commanded_gimbal[0].abs().max(v.commanded_gimbal[1].abs())) / 910.0)
        .clamp(0.0, 1.0);
    frame.render_widget(
        Gauge::default()
            .block(block("GIMBAL AUTHORITY USED"))
            .ratio(sat)
            .gauge_style(Style::default().fg(if sat > 0.9 {
                Color::Red
            } else {
                Color::Magenta
            })),
        rows[1],
    );
    frame.render_widget(Paragraph::new("Effector-neutral torque demand → capability-bound allocator → physical actuator.
Monitor-only Firestorm always emits neutral physical commands. Canard/RCS capability IDs remain reserved for Phase 9.5.").block(block("CONTROL CONTRACT")),rows[2]);
}
fn page_nav(frame: &mut Frame<'_>, area: Rect, model: &LocalConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        return;
    };
    let residual = [
        v.onboard_position_m[0] - v.ground_position_m[0],
        v.onboard_position_m[1] - v.ground_position_m[1],
        v.onboard_position_m[2] - v.ground_position_m[2],
    ];
    let velocity = [
        v.onboard_velocity_mps[0] - v.ground_velocity_mps[0],
        v.onboard_velocity_mps[1] - v.ground_velocity_mps[1],
        v.onboard_velocity_mps[2] - v.ground_velocity_mps[2],
    ];
    let text = format!(
        "ONBOARD POSITION  {}
GROUND POSITION   {}
POSITION RESIDUAL {}

ONBOARD VELOCITY  {}
GROUND VELOCITY   {}
VELOCITY RESIDUAL {}

IMU VALID {:02X}  AID VALID {:04X}
NAV CHECKSUM {:08X}",
        fmt3(v.onboard_position_m, "m"),
        fmt3(v.ground_position_m, "m"),
        fmt3(residual, "m"),
        fmt3(v.onboard_velocity_mps, "m/s"),
        fmt3(v.ground_velocity_mps, "m/s"),
        fmt3(velocity, "m/s"),
        v.inertial_validity,
        v.aid_validity,
        v.navigation_checksum
    );
    frame.render_widget(
        Paragraph::new(text).block(block("TRUTH-BLIND LOCAL-ENU NAVIGATION")),
        area,
    );
}
fn page_vehicle(frame: &mut Frame<'_>, area: Rect, model: &LocalConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        return;
    };
    let text = format!(
        "REFERENCE VEHICLE   FIRESTORM 54 / I211W{}
MASS                {:8.3} kg
THRUST              {:8.1} N
STATIC MARGIN       {:8.3} cal
ANGLE OF ATTACK     {:8.3} deg
DYNAMIC PRESSURE    {:8.1} Pa
WIND                {}

RAIL → POWERED 6-DOF → COAST → DROGUE → MAIN → GROUND
Avionics control retires attitude at first recovery deployment.",
        if v.commanded_gimbal != [0, 0] {
            " GIMBAL DERIVATIVE"
        } else {
            ""
        },
        v.mass_kg,
        v.thrust_n,
        v.static_margin,
        v.angle_of_attack_deg,
        v.dynamic_pressure_pa,
        fmt3(v.wind_mps, "m/s")
    );
    frame.render_widget(
        Paragraph::new(text).block(block("VEHICLE / ENVIRONMENT / RECOVERY")),
        area,
    );
}
fn page_link(frame: &mut Frame<'_>, area: Rect, model: &LocalConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        return;
    };
    let e = model.evidence;
    let text = format!(
        "KLF6 OUTER TRANSPORT / KLR8 CELLS
EPOCH              {}
SENSOR VALIDITY    IMU {:02X}  AID {:04X}
TRUTH CHAIN        {:08X}
NAV CHAIN          {:08X}
FLIGHT CHAIN       {:08X}
ALARMS             {:04X}

PLACEMENTS
  host world + host flight        READY
  host world + VICE/C64 flight    KLF6 EXACT-PACED
  combined stock-C64 loopback     SAME SERIALIZED CELL BOUNDARY

FINAL EVIDENCE     {}",
        v.epoch,
        v.inertial_validity,
        v.aid_validity,
        v.truth_checksum,
        v.navigation_checksum,
        v.flight_checksum,
        v.alarms,
        e.map(|x| format!("{:?} / {} releases", x.placement, x.releases))
            .unwrap_or_else(|| "IN PROGRESS".into())
    );
    frame.render_widget(
        Paragraph::new(text).block(block("LINK / EPOCHS / CHECKSUMS / DEADLINES")),
        area,
    );
}
fn page_truth(frame: &mut Frame<'_>, area: Rect, model: &LocalConsoleModel) {
    let Some(v) = model.latest.as_ref() else {
        return;
    };
    let text=format!("SIMULATION TRUTH — NOT AVAILABLE TO F1-F6 FLIGHT SOFTWARE

POSITION  {}
VELOCITY  {}
PHASE     {}
EVENTS    {:04X}
MASS      {:.3} kg
THRUST    {:.1} N
MACH      {:.4}
Q         {:.1} Pa
AOA       {:.4} deg
WIND      {}
TRUTH CRC {:08X}

F7 is an engineering comparison page. It is not launch approval, certification, or safety authority.",fmt3(v.truth_position_m,"m"),fmt3(v.truth_velocity_mps,"m/s"),v.phase,v.events,v.mass_kg,v.thrust_n,v.mach,v.dynamic_pressure_pa,v.angle_of_attack_deg,fmt3(v.wind_mps,"m/s"),v.truth_checksum);
    frame.render_widget(
        Paragraph::new(text).block(block("SIMULATION TRUTH / INJECTED WORLD")),
        area,
    );
}
pub fn render_local_console(frame: &mut Frame<'_>, model: &LocalConsoleModel, title: &str) {
    let area = header(frame, frame.area(), model, title);
    match model.page {
        0 => page_flight(frame, area, model),
        1 => page_trajectory(frame, area, model),
        2 => page_gnc(frame, area, model),
        3 => page_nav(frame, area, model),
        4 => page_vehicle(frame, area, model),
        5 => page_link(frame, area, model),
        _ => page_truth(frame, area, model),
    }
}
enum LiveEvent {
    Update(Phase85Update),
    Finish(Phase85RunEvidence),
    Failed,
}
struct ChannelSink {
    tx: Sender<LiveEvent>,
    pace: ConsolePace,
}
impl Phase85Sink for ChannelSink {
    fn publish(&mut self, u: &Phase85Update, _: &Kat8Frame) {
        let _ = self.tx.send(LiveEvent::Update(u.clone()));
        if self.pace == ConsolePace::Realtime {
            thread::sleep(Duration::from_micros(31_250));
        }
    }
    fn finish(&mut self, e: &Phase85RunEvidence) {
        let _ = self.tx.send(LiveEvent::Finish(*e));
    }
}
fn restore(terminal: &mut Terminal<CrosstermBackend<Stderr>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}
pub fn run_local_console_with_worker<F>(
    config: LocalConsoleConfig,
    worker: F,
) -> Result<Phase85RunEvidence, LocalConsoleError>
where
    F: FnOnce(&mut dyn Phase85Sink) -> Result<(), ()> + Send + 'static,
{
    let (tx, rx): (Sender<LiveEvent>, Receiver<LiveEvent>) = mpsc::channel();
    let worker_config = config.clone();
    thread::spawn(move || {
        let mut sink = ChannelSink {
            tx: tx.clone(),
            pace: worker_config.pace,
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
    let mut model = LocalConsoleModel::new();
    let mut answer = None;
    let result = 'console: loop {
        loop {
            match rx.try_recv() {
                Ok(LiveEvent::Update(update)) => model.accept(update),
                Ok(LiveEvent::Finish(evidence)) => {
                    model.complete = true;
                    model.evidence = Some(evidence);
                    answer = Some(evidence);
                }
                Ok(LiveEvent::Failed) | Err(TryRecvError::Disconnected) => {
                    break 'console Err(LocalConsoleError::Worker);
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        terminal.draw(|frame| render_local_console(frame, &model, &config.title))?;
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::F(number) if (1..=7).contains(&number) => {
                            model.page = (number - 1) as usize;
                        }
                        KeyCode::Left => model.page = (model.page + 6) % 7,
                        KeyCode::Right => model.page = (model.page + 1) % 7,
                        KeyCode::Char('q') | KeyCode::Esc if answer.is_some() => {
                            break 'console Ok(answer.unwrap());
                        }
                        _ => {}
                    }
                }
            }
        }
        if model.complete && config.pace == ConsolePace::Fast {
            break 'console Ok(answer.unwrap());
        }
    };
    restore(&mut terminal)?;
    result
}

pub fn run_local_console(
    config: LocalConsoleConfig,
) -> Result<Phase85RunEvidence, LocalConsoleError> {
    let gimbal = config.gimbal;
    run_local_console_with_worker(config, move |sink| {
        run_host_host(gimbal, Some(sink))
            .map(|_| ())
            .map_err(|_| ())
    })
}

pub fn recording_from_updates(
    updates: Vec<Phase85Update>,
    evidence: Phase85RunEvidence,
) -> Result<Kmr8Recording, LocalConsoleError> {
    let reference = checked_in_reference(false).map_err(|_| LocalConsoleError::Mission)?;
    Ok(Kmr8Recording {
        schema: "ksa64.kmr8-v1".into(),
        placement: evidence.placement,
        vehicle_identity: reference.vehicle.identity,
        avionics_identity: reference.avionics.identity,
        actuator_identity: reference.capability.identity,
        updates,
        terminal_checksum_chains: evidence.summary.checksum_chains,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    #[test]
    fn every_page_renders_at_full_and_compact_sizes() {
        let mut model = LocalConsoleModel::new();
        let mut collector = VecSink(Vec::new());
        let evidence = run_host_host(false, Some(&mut collector)).unwrap();
        for update in collector.0 {
            model.accept(update);
        }
        model.complete = true;
        model.evidence = Some(evidence);
        for size in [(140, 45), (80, 24)] {
            for (page, _) in PAGE_NAMES.iter().enumerate() {
                model.page = page;
                let backend = TestBackend::new(size.0, size.1);
                let mut terminal = Terminal::new(backend).unwrap();
                terminal
                    .draw(|f| render_local_console(f, &model, "KSA64 TEST"))
                    .unwrap();
                let content = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|c| c.symbol())
                    .collect::<String>();
                assert!(content.contains(PAGE_NAMES[page].split_whitespace().last().unwrap()));
            }
        }
    }
    struct VecSink(Vec<Phase85Update>);
    impl Phase85Sink for VecSink {
        fn publish(&mut self, u: &Phase85Update, _: &Kat8Frame) {
            self.0.push(u.clone());
        }
    }
}
