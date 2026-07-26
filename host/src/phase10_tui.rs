//! Seven-page live/replay Mission Control presentation for Phase 10.

use crate::phase10_mission::{
    capture_nominal_global_mission, q12, q14, q16, q21, q24, q28_radians_to_degrees,
    GlobalMissionCapture, GlobalMissionUpdate,
};
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
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

const PAGE_NAMES: [&str; 7] = [
    "F1 MISSION",
    "F2 TRACK",
    "F3 EARTH",
    "F4 NAV",
    "F5 CONTROL",
    "F6 MODELS",
    "F7 TRUTH",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalConsolePace {
    Fast,
    Realtime,
}

#[derive(Clone, Debug)]
pub struct GlobalConsoleConfig {
    pub title: String,
    pub pace: GlobalConsolePace,
    pub auto_exit: bool,
}

impl Default for GlobalConsoleConfig {
    fn default() -> Self {
        Self {
            title: "KSA64 // GLOBAL EARTH MISSION CONTROL".into(),
            pace: GlobalConsolePace::Fast,
            auto_exit: false,
        }
    }
}

#[derive(Debug)]
pub enum GlobalConsoleError {
    Io(io::Error),
    Worker,
    Mission,
}

impl From<io::Error> for GlobalConsoleError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Debug)]
pub struct GlobalConsoleModel {
    pub page: usize,
    pub latest: Option<GlobalMissionUpdate>,
    pub history: VecDeque<GlobalMissionUpdate>,
    pub events: VecDeque<String>,
    pub complete: bool,
}

impl GlobalConsoleModel {
    pub fn new() -> Self {
        Self {
            page: 0,
            latest: None,
            history: VecDeque::new(),
            events: VecDeque::new(),
            complete: false,
        }
    }

    pub fn accept(&mut self, update: GlobalMissionUpdate) {
        if update.frame.events != 0 {
            self.events.push_back(format!(
                "T+{:8.3}  EVENTS 0x{:04X}  {:?}",
                q16(update.frame.mission_time_q16),
                update.frame.events,
                update.frame.segment
            ));
        }
        if self
            .latest
            .is_some_and(|old| old.frame.transition_count != update.frame.transition_count)
        {
            self.events.push_back(format!(
                "T+{:8.3}  FRAME OWNER → {:?}",
                q16(update.frame.mission_time_q16),
                update.frame.frame
            ));
        }
        while self.events.len() > 12 {
            self.events.pop_front();
        }
        self.history.push_back(update);
        while self.history.len() > 8_192 {
            self.history.pop_front();
        }
        self.latest = Some(update);
    }
}

impl Default for GlobalConsoleModel {
    fn default() -> Self {
        Self::new()
    }
}

enum WorkerMessage {
    Update(Box<GlobalMissionUpdate>),
    Complete(Box<Result<GlobalMissionCapture, ()>>),
}

pub fn run_global_console(
    config: GlobalConsoleConfig,
) -> Result<GlobalMissionCapture, GlobalConsoleError> {
    let (sender, receiver) = mpsc::channel();
    let pace = config.pace;
    let worker = thread::spawn(move || {
        let capture = capture_nominal_global_mission(|update| {
            let _ = sender.send(WorkerMessage::Update(Box::new(*update)));
            if pace == GlobalConsolePace::Realtime {
                thread::sleep(Duration::from_millis(250));
            }
        })
        .map_err(|_| ());
        let _ = sender.send(WorkerMessage::Complete(Box::new(capture)));
    });
    let mut terminal = TerminalSession::new()?;
    let mut model = GlobalConsoleModel::new();
    let capture = console_loop(&mut terminal.terminal, &receiver, &mut model, &config)?;
    worker.join().map_err(|_| GlobalConsoleError::Worker)?;
    capture.ok_or(GlobalConsoleError::Mission)
}

fn console_loop(
    terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    receiver: &Receiver<WorkerMessage>,
    model: &mut GlobalConsoleModel,
    config: &GlobalConsoleConfig,
) -> Result<Option<GlobalMissionCapture>, GlobalConsoleError> {
    let mut capture = None;
    loop {
        loop {
            match receiver.try_recv() {
                Ok(WorkerMessage::Update(update)) => model.accept(*update),
                Ok(WorkerMessage::Complete(result)) => {
                    capture = Some((*result).map_err(|_| GlobalConsoleError::Mission)?);
                    model.complete = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if capture.is_none() {
                        return Err(GlobalConsoleError::Worker);
                    }
                    break;
                }
            }
        }
        terminal.draw(|frame| render_global_console(frame, model, &config.title))?;
        if model.complete && config.auto_exit {
            return Ok(capture);
        }
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc if model.complete => return Ok(capture),
                    KeyCode::Left => {
                        model.page = (model.page + PAGE_NAMES.len() - 1) % PAGE_NAMES.len()
                    }
                    KeyCode::Right => model.page = (model.page + 1) % PAGE_NAMES.len(),
                    KeyCode::F(number) if (1..=7).contains(&number) => {
                        model.page = usize::from(number - 1)
                    }
                    _ => {}
                }
            }
        }
    }
}

pub fn render_global_console(frame: &mut Frame<'_>, model: &GlobalConsoleModel, title: &str) {
    let body = header(frame, frame.area(), model, title);
    match model.page {
        0 => render_mission(frame, body, model),
        1 => render_track(frame, body, model),
        2 => render_earth(frame, body, model),
        3 => render_navigation(frame, body, model),
        4 => render_control(frame, body, model),
        5 => render_models(frame, body, model),
        _ => render_truth(frame, body, model),
    }
}

fn header(frame: &mut Frame<'_>, area: Rect, model: &GlobalConsoleModel, title: &str) -> Rect {
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
        "MISSION COMPLETE  F1-F7 / ←→ navigate  Q quit"
    } else {
        "LIVE GLOBAL EXECUTION  F1-F7 / ←→ navigate"
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

fn render_mission(frame: &mut Frame<'_>, area: Rect, model: &GlobalConsoleModel) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Length(3),
            Constraint::Min(6),
        ])
        .split(columns[0]);
    let Some(update) = model.latest else {
        frame.render_widget(waiting(), area);
        return;
    };
    let value = &update.frame;
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    format!("{:?}", value.segment),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    "  T+{:8.3}s  release {}",
                    q16(value.mission_time_q16),
                    update.release
                )),
            ]),
            Line::from(format!(
                "OWNER {:?}  FLIGHT MODE {}  TRANSITIONS {}",
                value.frame, value.flight_mode, value.transition_count
            )),
            Line::from(format!(
                "ALT {:9.3} km   MACH {:6.3}   Q {:9.1} Pa",
                q12(value.altitude_q12_km),
                q24(value.mach_q24),
                q14(value.dynamic_pressure_q14_pa)
            )),
            Line::from(format!(
                "MASS {:7.2} kg   MAIN {:7.2} kg   RCS {:5.2} kg",
                q21(value.total_mass_q21_kg),
                q21(value.main_propellant_q21_kg),
                q21(value.rcs_propellant_q21_kg)
            )),
            Line::from(format!(
                "ALARMS 0x{:04X}   EVENTS 0x{:04X}",
                value.alarms, value.events
            )),
        ])
        .block(block("FLIGHT DIRECTOR")),
        left[0],
    );
    let mission_fraction = (q16(value.mission_time_q16) / 2_700.0).clamp(0.0, 1.0);
    frame.render_widget(
        Gauge::default()
            .block(block("MISSION CLOCK / 45 MIN ENVELOPE"))
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(mission_fraction)
            .label(format!("{:5.1}%", mission_fraction * 100.0)),
        left[1],
    );
    frame.render_widget(
        Paragraph::new(
            model
                .events
                .iter()
                .cloned()
                .map(Line::from)
                .collect::<Vec<_>>(),
        )
        .block(block("EVENT / OWNERSHIP LOG"))
        .wrap(Wrap { trim: false }),
        left[2],
    );
    frame.render_widget(
        Paragraph::new(status_matrix(value))
            .block(block("SYSTEMS"))
            .wrap(Wrap { trim: false }),
        columns[1],
    );
}

fn render_track(frame: &mut Frame<'_>, area: Rect, model: &GlobalConsoleModel) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(area);
    let ground = model
        .history
        .iter()
        .map(|update| {
            (
                q28_radians_to_degrees(update.plot.longitude_q28_rad),
                q28_radians_to_degrees(update.plot.latitude_q28_rad),
                '*',
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(ascii_plot(
            &ground,
            rows[0].width.saturating_sub(2) as usize,
            rows[0].height.saturating_sub(2) as usize,
        ))
        .block(block("WORLD GROUND TRACK  longitude × latitude")),
        rows[0],
    );
    let altitude = model
        .history
        .iter()
        .map(|update| {
            (
                q16(update.frame.mission_time_q16),
                q12(update.frame.altitude_q12_km),
                '•',
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(ascii_plot(
            &altitude,
            rows[1].width.saturating_sub(2) as usize,
            rows[1].height.saturating_sub(2) as usize,
        ))
        .block(block("ALTITUDE PROFILE  time × km")),
        rows[1],
    );
}

fn render_earth(frame: &mut Frame<'_>, area: Rect, model: &GlobalConsoleModel) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
        .split(area);
    let inertial = model
        .history
        .iter()
        .map(|update| {
            (
                q12(update.frame.truth_position_q12[0]),
                q12(update.frame.truth_position_q12[1]),
                match update.frame.frame {
                    ksa64_core::phase10_contract::ReferenceFrameId::EarthInertialEciV1 => 'I',
                    ksa64_core::phase10_contract::ReferenceFrameId::EarthFixedEcefV1 => 'E',
                    _ => 'L',
                },
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(ascii_plot(
            &inertial,
            columns[0].width.saturating_sub(2) as usize,
            columns[0].height.saturating_sub(2) as usize,
        ))
        .block(block("ACTIVE-FRAME X/Y PROJECTION  L/E/I owner")),
        columns[0],
    );
    let text = model.latest.map_or_else(
        || "waiting…".to_owned(),
        |update| {
            format!(
                "FRAME OWNER\n{:?}\n\nSEGMENT\n{:?}\n\nGEODETIC\nlat {:+10.5}°\nlon {:+10.5}°\nalt {:10.3} km\n\nTRANSITIONS {}\n\nOwnership changes only on exact 32 Hz releases.",
                update.frame.frame,
                update.frame.segment,
                q28_radians_to_degrees(update.plot.latitude_q28_rad),
                q28_radians_to_degrees(update.plot.longitude_q28_rad),
                q12(update.plot.altitude_q12_km),
                update.frame.transition_count
            )
        },
    );
    frame.render_widget(
        Paragraph::new(text).block(block("EARTH / FRAME SERVICE")),
        columns[1],
    );
}

fn render_navigation(frame: &mut Frame<'_>, area: Rect, model: &GlobalConsoleModel) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(area);
    let Some(update) = model.latest else {
        frame.render_widget(waiting(), area);
        return;
    };
    let value = &update.frame;
    let position_error = [
        value.navigation_position_q12[0] - value.truth_position_q12[0],
        value.navigation_position_q12[1] - value.truth_position_q12[1],
        value.navigation_position_q12[2] - value.truth_position_q12[2],
    ];
    let velocity_error = [
        value.navigation_velocity_q24[0] - value.truth_velocity_q24[0],
        value.navigation_velocity_q24[1] - value.truth_velocity_q24[1],
        value.navigation_velocity_q24[2] - value.truth_velocity_q24[2],
    ];
    frame.render_widget(
        Paragraph::new(format!(
            "ONBOARD POSITION  km raw Q12\n{}\n\nGROUND/WORLD POSITION\n{}\n\nRESIDUAL\n{}\n\nONBOARD VELOCITY km/s\n{}\n\nRESIDUAL\n{}",
            format3(value.navigation_position_q12, 12),
            format3(value.truth_position_q12, 12),
            format3(position_error, 12),
            format3(value.navigation_velocity_q24, 24),
            format3(velocity_error, 24),
        ))
        .block(block("ONBOARD vs GROUND NAVIGATION")),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "NAV ATTITUDE Q30\n{:?}\n\nTRUTH ATTITUDE Q30\n{:?}\n\nCHECKSUMS\nnavigation  {:08X}\nsensors     {:08X}\nflight      {:08X}\n\nALARMS 0x{:04X}\nNo truth reset occurs at frame transitions.",
            value.navigation_attitude_q30,
            value.truth_attitude_q30,
            value.checksums[1],
            value.checksums[3],
            value.checksums[2],
            value.alarms,
        ))
        .block(block("ATTITUDE / AID INTEGRITY"))
        .wrap(Wrap { trim: false }),
        columns[1],
    );
}

fn render_control(frame: &mut Frame<'_>, area: Rect, model: &GlobalConsoleModel) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(8)])
        .split(area);
    let Some(update) = model.latest else {
        frame.render_widget(waiting(), area);
        return;
    };
    let value = &update.frame;
    frame.render_widget(
        Paragraph::new(format!(
            "GIMBAL PITCH {:+7.3}°  YAW {:+7.3}°\nRCS PULSES {:?}\nCOMMAND FLAGS 0x{:02X}  DISCRETE 0x{:02X}\nBODY RATE Q24 {:?}\nMAIN PROPELLANT {:8.3} kg  RCS RESERVE {:7.3} kg\nAuthority: gimbal powered flight • RCS coast • recovery safe",
            q15_degrees(value.gimbal_q15[0]),
            q15_degrees(value.gimbal_q15[1]),
            value.rcs_pulses,
            value.command_flags,
            value.command_discrete,
            value.truth_angular_rate_q24,
            q21(value.main_propellant_q21_kg),
            q21(value.rcs_propellant_q21_kg),
        ))
        .block(block("GUIDANCE / ATTITUDE / EFFECTORS")),
        rows[0],
    );
    let pulse_history: Vec<u64> = model
        .history
        .iter()
        .map(|sample| {
            sample
                .frame
                .rcs_pulses
                .iter()
                .map(|value| u64::from(*value))
                .sum::<u64>()
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Sparkline::default()
            .block(block("RCS PULSE ACTIVITY"))
            .data(&pulse_history)
            .style(Style::default().fg(Color::Magenta)),
        rows[1],
    );
}

fn render_models(frame: &mut Frame<'_>, area: Rect, model: &GlobalConsoleModel) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let latest = model.latest;
    let left = format!(
        "EARTH\nWGS 84 ellipsoid\nCentral gravity + J2\nIERS 2010 compiled orientation\nIAU 2006/2000A source model\n\nTIME\nUTC input epoch 2024-01-01\nTAI continuous integration\nTT and UT1 host services\nNo EOP/leap extrapolation\n\nACTIVE\nframe {:?}\nsegment {:?}",
        latest.map(|u| u.frame.frame),
        latest.map(|u| u.frame.segment)
    );
    frame.render_widget(
        Paragraph::new(left).block(block("EARTH / TIME AUTHORITY")),
        columns[0],
    );
    let right = format!(
        "ATMOSPHERE\nCompiled U.S. Standard Atmosphere 1976\nCo-rotating air + bounded wind profile\n\nENVELOPES\nMach ≤ 10\nAoA ≤ 15°\nDynamic pressure ≤ 100 kPa\nAltitude −1…2000 km\nDuration ≤ 4 h\n\nCURRENT\nMach {:8.4}\nQ {:10.1} Pa\nalt {:10.3} km",
        latest.map_or(0.0, |u| q24(u.frame.mach_q24)),
        latest.map_or(0.0, |u| q14(u.frame.dynamic_pressure_q14_pa)),
        latest.map_or(0.0, |u| q12(u.frame.altitude_q12_km)),
    );
    frame.render_widget(
        Paragraph::new(right).block(block("ENVIRONMENT / VALIDITY")),
        columns[1],
    );
}

fn render_truth(frame: &mut Frame<'_>, area: Rect, model: &GlobalConsoleModel) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    let Some(update) = model.latest else {
        frame.render_widget(waiting(), area);
        return;
    };
    let value = &update.frame;
    frame.render_widget(
        Paragraph::new(format!(
            "ACTIVE TRUTH POSITION Q12\n{:?}\n\nACTIVE TRUTH VELOCITY Q24\n{:?}\n\nECEF POSITION Q12\n{:?}\n\nECEF VELOCITY Q24\n{:?}\n\nATTITUDE Q30\n{:?}\n\nANGULAR RATE Q24\n{:?}",
            value.truth_position_q12,
            value.truth_velocity_q24,
            value.ecef_position_q12,
            value.ecef_velocity_q24,
            value.truth_attitude_q30,
            value.truth_angular_rate_q24,
        ))
        .block(block("SIMULATION TRUTH — RESTRICTED PAGE")),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "truth       {:08X}\nnavigation  {:08X}\nflight      {:08X}\nsensors     {:08X}\ncommand     {:08X}\nstatus      {:08X}\ndeadlines   {}\ncase seed   {:08X}\n\nFRAME {:?}\nSEGMENT {:?}\nEVENTS 0x{:04X}\n\nEngineering simulation only.\nNot certification or safety authority.",
            value.checksums[0],
            value.checksums[1],
            value.checksums[2],
            value.checksums[3],
            value.checksums[4],
            value.checksums[5],
            value.checksums[6],
            value.checksums[7],
            value.frame,
            value.segment,
            value.events,
        ))
        .block(block("INTEGRITY / INJECTED-FAULT VIEW")),
        columns[1],
    );
}

fn status_matrix(value: &ksa64_core::phase10_telemetry::GlobalTelemetryFrame) -> String {
    format!(
        "WORLD AUTHORITY      {:?}\nONBOARD NAVIGATION  {}\nFRAME SERVICE       {}\nGIMBAL              {}\nRCS                 {}\nRECOVERY            {}\nTELEMETRY           {}\n\nPHYSICAL CHECKSUM\n{:08X}\n\nFLIGHT CHECKSUM\n{:08X}",
        value.frame,
        if value.alarms & 0x20 == 0 { "NOMINAL" } else { "ALARM" },
        if value.alarms & 0x04 == 0 { "VALID" } else { "ALARM" },
        if value.main_propellant_q21_kg > 0 { "AVAILABLE" } else { "INACTIVE" },
        if value.rcs_propellant_q21_kg > 0 { "AVAILABLE" } else { "DEPLETED" },
        if value.command_discrete == 0 { "ARMED / STANDBY" } else { "COMMAND ACTIVE" },
        if value.alarms & 0x08 == 0 { "LINK GOOD" } else { "LINK ALARM" },
        value.checksums[0],
        value.checksums[2],
    )
}

fn block(title: &str) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn waiting() -> Paragraph<'static> {
    Paragraph::new("waiting for global mission telemetry…").block(block("MISSION CONTROL"))
}

fn format3(values: [i32; 3], bits: u8) -> String {
    let scale = (1u64 << bits) as f64;
    format!(
        "X {:+12.6}  Y {:+12.6}  Z {:+12.6}",
        f64::from(values[0]) / scale,
        f64::from(values[1]) / scale,
        f64::from(values[2]) / scale,
    )
}

fn q15_degrees(raw: i16) -> f64 {
    f64::from(raw) / 32_768.0 * 180.0 / std::f64::consts::PI
}

fn ascii_plot(points: &[(f64, f64, char)], width: usize, height: usize) -> String {
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
    if (max_x - min_x).abs() < f64::EPSILON {
        max_x = min_x + 1.0;
    }
    if (max_y - min_y).abs() < f64::EPSILON {
        max_y = min_y + 1.0;
    }
    let mut grid = vec![vec![' '; width]; height];
    for (x, y, symbol) in points {
        let column = (((x - min_x) / (max_x - min_x)) * (width - 1) as f64)
            .round()
            .clamp(0.0, (width - 1) as f64) as usize;
        let row = (height - 1)
            - ((((y - min_y) / (max_y - min_y)) * (height - 1) as f64)
                .round()
                .clamp(0.0, (height - 1) as f64) as usize);
        grid[row][column] = *symbol;
    }
    grid.into_iter()
        .map(|row| row.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stderr>>,
}

impl TerminalSession {
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stderr = io::stderr();
        execute!(stderr, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stderr))?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_core::phase10_contract::{GlobalSegment, ReferenceFrameId};
    use ksa64_core::phase10_telemetry::{GlobalPlotPoint, GlobalTelemetryFrame};
    use ratatui::backend::TestBackend;

    fn update() -> GlobalMissionUpdate {
        GlobalMissionUpdate {
            release: 32,
            frame: GlobalTelemetryFrame {
                step: 32,
                mission_time_q16: 1 << 16,
                frame: ReferenceFrameId::EarthFixedEcefV1,
                segment: GlobalSegment::EcefAscent,
                flight_mode: 2,
                events: 1,
                truth_position_q12: [26_000_000, -4_000_000, 12_000_000],
                truth_velocity_q24: [100, 200, 300],
                truth_attitude_q30: [1 << 30, 0, 0, 0],
                truth_angular_rate_q24: [1, 2, 3],
                ecef_position_q12: [26_000_000, -4_000_000, 12_000_000],
                ecef_velocity_q24: [100, 200, 300],
                navigation_position_q12: [26_000_001, -4_000_001, 12_000_001],
                navigation_velocity_q24: [101, 199, 301],
                navigation_attitude_q30: [1 << 30, 0, 0, 0],
                altitude_q12_km: 10 << 12,
                mach_q24: 2 << 24,
                dynamic_pressure_q14_pa: 20_000 << 14,
                total_mass_q21_kg: 400 << 21,
                main_propellant_q21_kg: 250 << 21,
                rcs_propellant_q21_kg: 5 << 21,
                gimbal_q15: [10, -10],
                rcs_pulses: [0; 12],
                command_flags: 1,
                command_discrete: 0,
                alarms: 0,
                transition_count: 1,
                checksums: [1, 2, 3, 4, 5, 6, 0, 8],
            },
            plot: GlobalPlotPoint {
                mission_time_q16: 1 << 16,
                latitude_q28_rad: 130_000_000,
                longitude_q28_rad: -370_000_000,
                altitude_q12_km: 10 << 12,
                downrange_q12_km: 2 << 12,
                crossrange_q12_km: 1 << 12,
                speed_q24_km_s: 2 << 24,
                frame: ReferenceFrameId::EarthFixedEcefV1,
                segment: GlobalSegment::EcefAscent,
                events: 1,
                truth_checksum: 1,
            },
        }
    }

    #[test]
    fn every_page_renders_at_full_and_compact_sizes() {
        for (width, height) in [(120, 42), (80, 24)] {
            for page in 0..7 {
                let backend = TestBackend::new(width, height);
                let mut terminal = Terminal::new(backend).unwrap();
                let mut model = GlobalConsoleModel::new();
                model.page = page;
                model.accept(update());
                terminal
                    .draw(|frame| render_global_console(frame, &model, "PHASE 10 TEST"))
                    .unwrap();
            }
        }
    }
}
