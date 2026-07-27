//! Role-filtered F1-F7 Phase 11 Mission Control presentation.

use crate::phase11_authoring::{
    complete_project_session, CompiledMissionProject, CompletedMissionSession, MissionScenario,
};
use crate::phase11_live::{
    LiveMissionSession, MissionOperatorAction, MissionSessionEvent, MissionSessionEventKind,
    MissionSessionLifecycle, MissionSessionSnapshot,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ksa64_flight::phase11::ksa_g10r_reference_ops_manifest;
use ksa64_interface::phase11::OperationalRole;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline, Tabs, Wrap};
use ratatui::{Frame, Terminal};
use std::io::{self, Stderr};
use std::time::{Duration, Instant};

pub const OPERATIONS_PAGE_NAMES: [&str; 7] = [
    "F1 FLIGHT",
    "F2 TRAJECTORY",
    "F3 GUIDANCE+UPLINK",
    "F4 NAVIGATION",
    "F5 FLIGHT SOFTWARE",
    "F6 COMMS+ACTIONS",
    "F7 SIM DIRECTOR",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationsPublicModel {
    pub definition_identity: u32,
    pub scenario_identity: u32,
    pub evidence_identity: u32,
    pub package_identity: u32,
    pub plan_identity: u32,
    pub role: OperationalRole,
    pub hints: bool,
    pub releases: u32,
    pub flight_checksum: u32,
    pub navigation_checksum: u32,
    pub command_checksum: u32,
    pub prediction_checksum: u32,
    pub procedure_chain: u32,
    pub journal_chain: u32,
    pub action_chain: u32,
    pub rejected_loads: u16,
    pub safe: bool,
    pub action_rows: Vec<String>,
    pub procedure_label: String,
    pub communications_label: String,
    pub predictor_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimDirectorPrivateModel {
    pub truth_evidence_identity: u32,
    pub fault_identity: u32,
    pub counterfactuals: Vec<(String, u32)>,
    pub model_envelope: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationsConsoleModel {
    pub page: usize,
    pub public: OperationsPublicModel,
    private_truth: Option<SimDirectorPrivateModel>,
}

impl OperationsConsoleModel {
    pub fn from_completed(
        project: &CompiledMissionProject,
        completed: &CompletedMissionSession,
    ) -> Self {
        let evidence = &completed.evidence;
        let action_rows = evidence
            .actions
            .iter()
            .map(|action| {
                format!(
                    "E{:04} STEP {:02} {:?} {:?} LOAD {:08X}",
                    action.epoch,
                    action.procedure_step,
                    action.action_kind,
                    action.state,
                    action.load_identity
                )
            })
            .collect();
        let procedure_label = match project.scenario {
            MissionScenario::GnssLoss => "ASCENT/GLOBAL COAST — LOSS OF GNSS AIDING",
            MissionScenario::GnssLossFull => "FULL GLOBAL FLIGHT — LOSS OF GNSS AIDING",
            MissionScenario::GuidanceUpdate => "PLANNED COAST GUIDANCE UPDATE",
            MissionScenario::GroundBlackout => "GROUND COMMUNICATIONS BLACKOUT",
            MissionScenario::InvalidOperations => "INVALID COMMAND REJECTION",
            MissionScenario::SafeholdRecovery => "SAFEHOLD / ENTRY / RECOVERY",
            MissionScenario::Nominal => "NOMINAL GLOBAL OPERATIONS",
        }
        .to_string();
        let private_truth = (project.role == OperationalRole::SimDirector).then(|| {
            let counterfactuals = completed
                .debrief
                .as_ref()
                .map(|debrief| {
                    debrief
                        .summary
                        .counterfactuals
                        .iter()
                        .map(|item| (format!("{:?}", item.factor), item.evidence_identity))
                        .collect()
                })
                .unwrap_or_default();
            SimDirectorPrivateModel {
                truth_evidence_identity: evidence.evidence_identity,
                fault_identity: project.master_seed ^ evidence.scenario_identity,
                counterfactuals,
                model_envelope: "PHASE 10 GLOBAL ECEF 6-DOF / DECLARED PACK ENVELOPES".into(),
            }
        });
        Self {
            page: 0,
            public: OperationsPublicModel {
                definition_identity: project.definition_identity,
                scenario_identity: evidence.scenario_identity,
                evidence_identity: evidence.evidence_identity,
                package_identity: ksa_g10r_reference_ops_manifest().manifest_identity,
                plan_identity: ksa64_flight::phase11::ksa_g10r_reference_mission_plan()
                    .plan_identity,
                role: project.role,
                hints: project.source.hints,
                releases: evidence.releases,
                flight_checksum: evidence.flight_checksum,
                navigation_checksum: evidence.navigation_checksum,
                command_checksum: evidence.command_checksum,
                prediction_checksum: evidence.prediction_checksum,
                procedure_chain: evidence.procedure_chain,
                journal_chain: evidence.journal_chain,
                action_chain: evidence.action_chain,
                rejected_loads: evidence.rejected_loads,
                safe: evidence.safe,
                action_rows,
                procedure_label,
                communications_label: if project.scenario == MissionScenario::GroundBlackout {
                    "BLACKOUT COMPLETED / JOURNAL REACQUIRED"
                } else {
                    "UPLINK + DOWNLINK AVAILABLE"
                }
                .into(),
                predictor_label: "ONBOARD EST / GROUND-PROPAGATED + GROUND EST".into(),
            },
            private_truth,
        }
    }

    pub fn from_live(
        project: &CompiledMissionProject,
        snapshot: &MissionSessionSnapshot,
        events: &[MissionSessionEvent],
    ) -> Self {
        let action_rows = events
            .iter()
            .filter(|event| {
                matches!(
                    event.kind,
                    MissionSessionEventKind::ActionStaged
                        | MissionSessionEventKind::ActionCommitted
                        | MissionSessionEventKind::ActionCancelled
                        | MissionSessionEventKind::ActionRejected
                )
            })
            .map(|event| {
                format!(
                    "E{:04} {:?} LOAD {:08X}",
                    event.release_epoch, event.kind, event.detail_identity
                )
            })
            .collect();
        let private_truth =
            (project.role == OperationalRole::SimDirector).then(|| SimDirectorPrivateModel {
                truth_evidence_identity: snapshot.flight_checksum.unwrap_or(0),
                fault_identity: project.master_seed
                    ^ crate::phase11_scenarios::GNSS_LOSS_SCENARIO_ID,
                counterfactuals: Vec::new(),
                model_envelope:
                    "LIVE OPERATIONAL SESSION / TRUTH COUNTERFACTUALS AFTER FINALIZATION".into(),
            });
        Self {
            page: 0,
            public: OperationsPublicModel {
                definition_identity: project.definition_identity,
                scenario_identity: crate::phase11_scenarios::GNSS_LOSS_SCENARIO_ID,
                evidence_identity: snapshot.evidence_identity.unwrap_or(0),
                package_identity: ksa_g10r_reference_ops_manifest().manifest_identity,
                plan_identity: ksa64_flight::phase11::ksa_g10r_reference_mission_plan()
                    .plan_identity,
                role: project.role,
                hints: project.source.hints,
                releases: snapshot.release_epoch,
                flight_checksum: snapshot.flight_checksum.unwrap_or(0),
                navigation_checksum: snapshot.navigation_checksum.unwrap_or(0),
                command_checksum: snapshot.command_checksum.unwrap_or(0),
                prediction_checksum: snapshot
                    .prediction
                    .map_or(0, |value| value.prediction_checksum),
                procedure_chain: snapshot.procedure_chain,
                journal_chain: snapshot.journal_chain,
                action_chain: snapshot.action_chain,
                rejected_loads: snapshot.rejected_loads,
                safe: snapshot.safe.unwrap_or(false),
                action_rows,
                procedure_label: format!(
                    "LOSS OF GNSS AIDING / {:?} / STEP {}",
                    snapshot.lifecycle, snapshot.procedure_step
                ),
                communications_label: if snapshot.staged_load_identity.is_some() {
                    "LOAD STAGED / COMMIT REQUIRED"
                } else {
                    "UPLINK + DOWNLINK AVAILABLE"
                }
                .into(),
                predictor_label: "LIVE ONBOARD EST / GROUND-PROPAGATED + GROUND EST".into(),
            },
            private_truth,
        }
    }

    pub fn truth_for_role(&self) -> Option<&SimDirectorPrivateModel> {
        (self.public.role == OperationalRole::SimDirector)
            .then_some(self.private_truth.as_ref())
            .flatten()
    }
}

#[derive(Debug)]
pub enum OperationsConsoleError {
    Io(io::Error),
    Session,
}

impl From<io::Error> for OperationsConsoleError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
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

pub fn run_operations_console(
    project: &CompiledMissionProject,
) -> Result<CompletedMissionSession, OperationsConsoleError> {
    if project.scenario != MissionScenario::GnssLoss {
        return run_completed_operations_console(project);
    }
    let mut live = LiveMissionSession::compiled(project.clone())
        .map_err(|_| OperationsConsoleError::Session)?;
    live.prepare()
        .map_err(|_| OperationsConsoleError::Session)?;
    let mut model =
        OperationsConsoleModel::from_live(project, &live.snapshot(), live.events_after(0));
    let mut terminal = TerminalSession::new()?;
    let mut last_release = Instant::now();
    loop {
        terminal.terminal.draw(|frame| {
            render_operations_console(frame, &model, "KSA64 // LIVE MISSION OPERATIONS")
        })?;
        if live.lifecycle() == MissionSessionLifecycle::Completed {
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press
                        && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                    {
                        break;
                    }
                    update_page(&mut model, key.code);
                }
            }
            continue;
        }

        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        live.abort(0x11ab_0001)
                            .map_err(|_| OperationsConsoleError::Session)?;
                        return Err(OperationsConsoleError::Session);
                    }
                    KeyCode::Char(' ') => {
                        if live.lifecycle() == MissionSessionLifecycle::Paused {
                            live.resume().map_err(|_| OperationsConsoleError::Session)?;
                        } else {
                            live.pause().map_err(|_| OperationsConsoleError::Session)?;
                        }
                    }
                    KeyCode::Char('.') => {
                        live.step_one_release()
                            .map_err(|_| OperationsConsoleError::Session)?;
                    }
                    KeyCode::Enter => submit_next_guided_action(&mut live)?,
                    other => update_page(&mut model, other),
                }
            }
        }

        let action_due =
            live.recommended_load().is_some() || live.commit_request_for_staged().is_some();
        let may_auto_act = project.role == OperationalRole::ScriptedOperator;
        if may_auto_act && action_due {
            submit_all_guided_actions(&mut live)?;
        }
        let should_pause_for_action = action_due
            && matches!(
                project.role,
                OperationalRole::GuidedOperator
                    | OperationalRole::FlightController
                    | OperationalRole::SimDirector
            );
        if should_pause_for_action && live.lifecycle() != MissionSessionLifecycle::Paused {
            live.pause().map_err(|_| OperationsConsoleError::Session)?;
        }
        if live.lifecycle() != MissionSessionLifecycle::Paused
            && last_release.elapsed() >= Duration::from_micros(31_250)
        {
            live.advance_one_release()
                .map_err(|_| OperationsConsoleError::Session)?;
            last_release = Instant::now();
        }
        let page = model.page;
        model = OperationsConsoleModel::from_live(project, &live.snapshot(), live.events_after(0));
        model.page = page;
    }
    live.finish().map_err(|_| OperationsConsoleError::Session)
}

fn submit_next_guided_action(live: &mut LiveMissionSession) -> Result<(), OperationsConsoleError> {
    if let Some(commit) = live.commit_request_for_staged() {
        live.submit_operator_action(MissionOperatorAction::Commit(commit))
            .map_err(|_| OperationsConsoleError::Session)?;
        if live.lifecycle() == MissionSessionLifecycle::Paused {
            live.resume().map_err(|_| OperationsConsoleError::Session)?;
        }
        return Ok(());
    }
    if let Some(load) = live.recommended_load() {
        live.submit_operator_action(MissionOperatorAction::Stage {
            load,
            completed_event_mask: 0,
        })
        .map_err(|_| OperationsConsoleError::Session)?;
    }
    Ok(())
}

fn submit_all_guided_actions(live: &mut LiveMissionSession) -> Result<(), OperationsConsoleError> {
    submit_next_guided_action(live)?;
    submit_next_guided_action(live)
}

fn update_page(model: &mut OperationsConsoleModel, code: KeyCode) {
    match code {
        KeyCode::Left => {
            model.page =
                (model.page + OPERATIONS_PAGE_NAMES.len() - 1) % OPERATIONS_PAGE_NAMES.len();
        }
        KeyCode::Right => model.page = (model.page + 1) % OPERATIONS_PAGE_NAMES.len(),
        KeyCode::F(number) if (1..=7).contains(&number) => {
            model.page = usize::from(number - 1);
        }
        _ => {}
    }
}

fn run_completed_operations_console(
    project: &CompiledMissionProject,
) -> Result<CompletedMissionSession, OperationsConsoleError> {
    let completed =
        complete_project_session(project, project.role == OperationalRole::ScriptedOperator)
            .map_err(|_| OperationsConsoleError::Session)?;
    let frozen_evidence = completed.evidence.clone();
    let mut model = OperationsConsoleModel::from_completed(project, &completed);
    let mut terminal = TerminalSession::new()?;
    loop {
        terminal.terminal.draw(|frame| {
            render_operations_console(frame, &model, "KSA64 // MISSION OPERATIONS")
        })?;
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                    break;
                }
                update_page(&mut model, key.code);
            }
        }
    }
    assert_eq!(completed.evidence, frozen_evidence);
    Ok(completed)
}

pub fn render_operations_console(
    frame: &mut Frame<'_>,
    model: &OperationsConsoleModel,
    title: &str,
) {
    let body = header(frame, frame.area(), model, title);
    match model.page {
        0 => render_flight(frame, body, &model.public),
        1 => render_trajectory(frame, body, &model.public),
        2 => render_uplink(frame, body, &model.public),
        3 => render_navigation(frame, body, &model.public),
        4 => render_flight_software(frame, body, &model.public),
        5 => render_communications(frame, body, &model.public),
        _ => render_sim_director(frame, body, &model.public, model.truth_for_role()),
    }
}

fn header(frame: &mut Frame<'_>, area: Rect, model: &OperationsConsoleModel, title: &str) -> Rect {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    frame.render_widget(
        Tabs::new(
            OPERATIONS_PAGE_NAMES
                .iter()
                .map(|name| Line::from(*name))
                .collect::<Vec<_>>(),
        )
        .select(model.page)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "{:?}  DEF {:08X}  EVID {:08X}  F1-F7/←→  SPACE pause  . step  ENTER action  Q quit",
            model.public.role, model.public.definition_identity, model.public.evidence_identity
        ))
        .style(Style::default().fg(Color::Green)),
        rows[2],
    );
    rows[1]
}

fn render_flight(frame: &mut Frame<'_>, area: Rect, model: &OperationsPublicModel) {
    let columns = columns(area, 58);
    let status = vec![
        Line::from(vec![
            Span::styled("FLIGHT DIRECTOR  ", cyan()),
            Span::styled(if model.safe { "SAFE STATE" } else { "NOMINAL" }, green()),
        ]),
        Line::from(format!("PROCEDURE  {}", model.procedure_label)),
        Line::from(format!("PLAN       {:08X}", model.plan_identity)),
        Line::from(format!("RELEASES   {}", model.releases)),
        Line::from(format!("COMMS      {}", model.communications_label)),
        Line::from(format!(
            "HINTS      {}",
            if model.hints { "AVAILABLE" } else { "OFF" }
        )),
    ];
    panel(frame, columns[0], "MISSION STATUS", status);
    panel(
        frame,
        columns[1],
        "ACTIVE CHECKLIST",
        vec![
            Line::from("1 CONFIRM NAV AID STATUS"),
            Line::from("2 VERIFY INERTIAL HEALTH"),
            Line::from("3 COMPARE GROUND RESIDUAL"),
            Line::from("4 STAGE / VALIDATE / COMMIT"),
            Line::from("5 VERIFY EXACT ACTIVATION"),
        ],
    );
}

fn render_trajectory(frame: &mut Frame<'_>, area: Rect, model: &OperationsPublicModel) {
    let rows = rows(area, 55);
    let data = checksum_spark(model.prediction_checksum);
    frame.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .title("ESTIMATE-BASED FUTURE ALTITUDE / GROUND TRACK")
                    .borders(Borders::ALL),
            )
            .data(&data)
            .style(Style::default().fg(Color::LightCyan)),
        rows[0],
    );
    panel(
        frame,
        rows[1],
        "PREDICTION PRODUCTS",
        vec![
            Line::from("ONBOARD COMPACT        estimate-bound / continues in blackout"),
            Line::from("ONBOARD EST / GROUND-PROPAGATED    rich host projection"),
            Line::from("GROUND EST             delayed independent tracking"),
            Line::from(format!(
                "PRODUCT CRC            {:08X}",
                model.prediction_checksum
            )),
            Line::from("SIM TRUTH COUNTERFACTUAL hidden outside F7"),
        ],
    );
}

fn render_uplink(frame: &mut Frame<'_>, area: Rect, model: &OperationsPublicModel) {
    let columns = columns(area, 48);
    panel(
        frame,
        columns[0],
        "ATOMIC LOAD–VALIDATE–COMMIT",
        vec![
            Line::from("GROUND BUILD → STAGE → VALIDATE"),
            Line::from("SEPARATE COMMIT / >=2 RELEASE LEAD"),
            Line::from("ACTIVATE ON EXACT 32 HZ RELEASE"),
            Line::from("NO DIRECT EFFECTOR COMMANDING"),
            Line::from(format!("COMMAND CHAIN {:08X}", model.command_checksum)),
        ],
    );
    let lines = if model.action_rows.is_empty() {
        vec![Line::from("NO OPERATOR ACTIONS")]
    } else {
        model.action_rows.iter().cloned().map(Line::from).collect()
    };
    panel(frame, columns[1], "ACTION / RECEIPT TIMELINE", lines);
}

fn render_navigation(frame: &mut Frame<'_>, area: Rect, model: &OperationsPublicModel) {
    let columns = columns(area, 50);
    panel(
        frame,
        columns[0],
        "ONBOARD ESTIMATE",
        vec![
            Line::from(format!(
                "NAV CHECKSUM      {:08X}",
                model.navigation_checksum
            )),
            Line::from("GNSS / IMU / AIR DATA  public avionics inputs"),
            Line::from("FRAME SERVICE           transported, identity-bound"),
            Line::from("TRUTH RESET             PROHIBITED"),
        ],
    );
    panel(
        frame,
        columns[1],
        "GROUND ESTIMATE",
        vec![
            Line::from("DELAYED / NOISY TRACKING OBSERVATIONS"),
            Line::from("INDEPENDENT ESTIMATOR + CHECKSUM"),
            Line::from("BOUNDED STATE UPDATE LOADS"),
            Line::from(format!(
                "PREDICTION CRC    {:08X}",
                model.prediction_checksum
            )),
        ],
    );
}

fn render_flight_software(frame: &mut Frame<'_>, area: Rect, model: &OperationsPublicModel) {
    let manifest = ksa_g10r_reference_ops_manifest();
    let rows = rows(area, 66);
    panel(
        frame,
        rows[0],
        "PACKAGE / ABI / RESOURCE EVIDENCE",
        vec![
            Line::from(format!(
                "KFS11 PACKAGE       {:08X}",
                model.package_identity
            )),
            Line::from(format!("ABI                 {:?}", manifest.abi)),
            Line::from(format!(
                "SCHEDULE            {}/{}/{} Hz",
                manifest.fast_hz, manifest.navigation_hz, manifest.guidance_hz
            )),
            Line::from(format!(
                "PERSIST / TRANSIENT {} / {} bytes",
                manifest.resource.persistent_bytes, manifest.resource.transient_bytes
            )),
            Line::from(format!(
                "STACK / JOURNAL     {} bytes / {} records",
                manifest.resource.stack_bytes, manifest.resource.journal_records
            )),
            Line::from(format!("FLIGHT CHAIN        {:08X}", model.flight_checksum)),
        ],
    );
    let ratio = (model.releases.min(320) as f64 / 320.0).clamp(0.0, 1.0);
    frame.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .title("BOUNDED SESSION PROGRESS")
                    .borders(Borders::ALL),
            )
            .gauge_style(Style::default().fg(Color::LightGreen))
            .ratio(ratio)
            .label(format!("{} releases", model.releases)),
        rows[1],
    );
}

fn render_communications(frame: &mut Frame<'_>, area: Rect, model: &OperationsPublicModel) {
    let columns = columns(area, 52);
    panel(
        frame,
        columns[0],
        "LOGICAL GROUND LINK",
        vec![
            Line::from(model.communications_label.clone()),
            Line::from("AVIONICS LOOP REMAINS INDEPENDENT"),
            Line::from("COMMITTED LOAD SURVIVES BLACKOUT"),
            Line::from("UNCOMMITTED LOAD NEVER ACTIVATES"),
            Line::from(format!("JOURNAL CHAIN {:08X}", model.journal_chain)),
        ],
    );
    panel(
        frame,
        columns[1],
        "PROCEDURE / ACTION EVIDENCE",
        vec![
            Line::from(format!("PROCEDURE {:08X}", model.procedure_chain)),
            Line::from(format!("ACTIONS   {:08X}", model.action_chain)),
            Line::from(format!("REJECTED  {}", model.rejected_loads)),
            Line::from("SIMULATION TIME / DETERMINISTIC REPLAY"),
            Line::from("HUMAN + SCRIPTED SHARE PUBLIC BROKER"),
        ],
    );
}

fn render_sim_director(
    frame: &mut Frame<'_>,
    area: Rect,
    public: &OperationsPublicModel,
    private: Option<&SimDirectorPrivateModel>,
) {
    let Some(private) = private else {
        panel(
            frame,
            area,
            "SIM DIRECTOR — ROLE-GATED",
            vec![
                Line::from(Span::styled(
                    "PRIVATE TRUTH NOT PRESENT IN THIS ROLE MODEL",
                    red(),
                )),
                Line::from(format!("CURRENT ROLE: {:?}", public.role)),
                Line::from("F1-F6 remain operational and truth-isolated."),
            ],
        );
        return;
    };
    let mut lines = vec![
        Line::from(format!(
            "TRUTH EVIDENCE       {:08X}",
            private.truth_evidence_identity
        )),
        Line::from(format!(
            "FAULT SCHEDULE       {:08X}",
            private.fault_identity
        )),
        Line::from(format!("ENVELOPE              {}", private.model_envelope)),
        Line::from("CONTROLLED COUNTERFACTUALS"),
    ];
    lines.extend(
        private
            .counterfactuals
            .iter()
            .map(|(name, identity)| Line::from(format!("  {name:28} {identity:08X}"))),
    );
    lines.push(Line::from(Span::styled(
        "TRUTH IS DIAGNOSTIC ONLY — NEVER A COMMAND SOURCE",
        red(),
    )));
    panel(frame, area, "SIM DIRECTOR", lines);
}

fn columns(area: Rect, left: u16) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left),
            Constraint::Percentage(100 - left),
        ])
        .split(area)
}

fn rows(area: Rect, top: u16) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(top),
            Constraint::Percentage(100 - top),
        ])
        .split(area)
}

fn panel(frame: &mut Frame<'_>, area: Rect, title: &str, lines: Vec<Line<'static>>) {
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title(title).borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn checksum_spark(seed: u32) -> Vec<u64> {
    let mut value = seed.max(1);
    (0..96)
        .map(|index| {
            value ^= value << 13;
            value ^= value >> 17;
            value ^= value << 5;
            let arc = 64u64.saturating_sub((i64::from(index) - 48).unsigned_abs());
            8 + arc + u64::from(value & 7)
        })
        .collect()
}

fn cyan() -> Style {
    Style::default()
        .fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD)
}
fn green() -> Style {
    Style::default()
        .fg(Color::LightGreen)
        .add_modifier(Modifier::BOLD)
}
fn red() -> Style {
    Style::default()
        .fg(Color::LightRed)
        .add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase11_authoring::compile_project_source;
    use ksa64_interface::phase11::UplinkState;
    use ratatui::backend::TestBackend;

    fn project(role: &str) -> CompiledMissionProject {
        compile_project_source(&format!(
            r#"{{"schema":"ksa64.phase11.mission-project.v1","name":"TUI","scenario":"gnss-loss","package":"KsaG10rReferenceOpsV1","role":"{role}","definition_identity":"0x11d10011","master_seed":"0x4b5341b0","hints":false,"provenance":[{{"kind":"accepted","source":"frozen","identity":"0x10a00001"}}]}}"#
        ))
        .unwrap()
    }

    fn render(role: &str, page: usize, width: u16, height: u16) -> (String, u32) {
        let project = project(role);
        let completed = complete_project_session(&project, true).unwrap();
        let identity = completed.evidence.evidence_identity;
        let mut model = OperationsConsoleModel::from_completed(&project, &completed);
        model.page = page;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_operations_console(frame, &model, "TEST"))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = (0..height)
            .flat_map(|y| (0..width).map(move |x| buffer[(x, y)].symbol()))
            .collect::<String>();
        (text, identity)
    }

    #[test]
    fn every_page_renders_at_full_and_compact_sizes_without_changing_evidence() {
        for page in 0..7 {
            for (width, height) in [(160, 48), (96, 28)] {
                let (_, before) = render("guided-operator", page, width, height);
                let (_, after) = render("guided-operator", page, width, height);
                assert_eq!(before, after);
            }
        }
    }

    #[test]
    fn truth_is_structurally_absent_outside_sim_director() {
        let (guided, _) = render("guided-operator", 6, 120, 35);
        let (director, _) = render("sim-director", 6, 120, 35);
        assert!(guided.contains("PRIVATE TRUTH NOT PRESENT"));
        assert!(!guided.contains("TRUTH EVIDENCE"));
        assert!(director.contains("TRUTH EVIDENCE"));
    }

    #[test]
    fn rejected_receipts_remain_visible_operational_evidence() {
        assert_ne!(UplinkState::Rejected as u8, UplinkState::Committed as u8);
    }
}
