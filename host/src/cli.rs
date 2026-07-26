//! Unified `ksa64` command surface.

use crate::application::{
    ApplicationError, ApplicationOutcome, ApplicationRequest, AuditRequest, CampaignRequest,
    EvidenceRequest, Ksa64Application, MissionApplicationRequest, MissionDisplay, MissionPace,
    MissionRequest, OptimizationRequest, ProjectRequest, TargetRequest,
};
use crate::optimization_app::{
    compile_optimization_manifest, run_optimization_manifest, serve_product_optimizer,
};
use crate::phase2::{capture_phase2_mission, format_phase2_inspection, inspect_phase2_stream};
use crate::product::{EvidenceMaturity, ExperienceDescriptor, ProductCatalog};
use crate::{capture_mission, format_inspection, inspect_stream};
use clap::{Parser, Subcommand, ValueEnum};
use ksa64_core::phase2_scenario::parse_phase2_scenario;
use ksa64_core::phase9_contract::{SearchEngineId, SearchPresetId};
use ksa64_core::scenario::parse_scenario_image;
use serde_json::json;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const LEGACY_SCENARIO: &[u8; 76] = include_bytes!("../../phase0/numeric/scenario-v1.bin");
const LEGACY_PHASE2_SCENARIO: &[u8; 884] = include_bytes!("../../phase2/examples/ksa2a-200km.ksc2");

#[derive(Parser, Debug)]
#[command(
    name = "ksa64",
    version,
    about = "KSA64 deterministic aerospace simulation and mission operations",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Emit deterministic structured output.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Discover current experiences and historical engineering tools.
    Catalog {
        #[command(subcommand)]
        command: CatalogCommand,
    },
    /// Compile and run Phase 11 mission projects.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Run missions and Mission Control.
    Mission {
        #[command(subcommand)]
        command: MissionCommand,
    },
    /// Run deterministic uncertainty campaigns.
    Campaign {
        #[command(subcommand)]
        command: CampaignCommand,
    },
    /// Compile or run robust design searches.
    Optimize {
        #[command(subcommand)]
        command: OptimizeCommand,
    },
    /// Inspect, verify, replay, or debrief evidence.
    Evidence {
        #[command(subcommand)]
        command: EvidenceCommand,
    },
    /// Discover, build, verify, or explicitly probe C64 targets.
    Target {
        #[command(subcommand)]
        command: TargetCommand,
    },
    /// Access frozen historical audits.
    Audit {
        #[command(subcommand)]
        command: AuditCommand,
    },
    #[command(hide = true)]
    Capture { path: PathBuf },
    #[command(name = "inspect", hide = true)]
    Inspect { path: PathBuf },
    #[command(name = "phase2-capture", hide = true)]
    Phase2Capture { path: PathBuf },
    #[command(name = "phase2-inspect", hide = true)]
    Phase2Inspect { path: PathBuf },
}

#[derive(Subcommand, Debug)]
pub enum CatalogCommand {
    List {
        #[arg(long)]
        historical: bool,
    },
    Show {
        id: String,
    },
    Export {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        historical: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    Lint {
        source: PathBuf,
    },
    Compile {
        source: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Run {
        source: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Script {
        source: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum MissionCommand {
    Run {
        id: String,
        #[arg(long)]
        scenario: Option<String>,
        #[arg(long)]
        role: Option<String>,
        #[arg(long, value_enum, default_value_t = DisplayArg::Summary)]
        display: DisplayArg,
        #[arg(long, value_enum, default_value_t = PaceArg::Fast)]
        pace: PaceArg,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Control {
        id: String,
        #[arg(long)]
        scenario: Option<String>,
        #[arg(long)]
        role: Option<String>,
        #[arg(long, value_enum, default_value_t = PaceArg::Fast)]
        pace: PaceArg,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum CampaignCommand {
    Run {
        id: String,
        #[arg(long, default_value = "routine")]
        preset: String,
        #[arg(long, default_value_t = 64)]
        runs: u32,
        #[arg(long, default_value_t = 1)]
        workers: usize,
        #[arg(long, default_value = "ksa64-campaign")]
        output: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum OptimizeCommand {
    Compile {
        source: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Run {
        id: String,
        #[arg(long, value_enum, default_value_t = EngineArg::Nsga2)]
        engine: EngineArg,
        #[arg(long, value_enum, default_value_t = PresetArg::Quick)]
        preset: PresetArg,
        #[arg(long, default_value_t = 1)]
        workers: usize,
        #[arg(long, default_value = "ksa64-optimization")]
        output: PathBuf,
        #[arg(long)]
        tui: bool,
    },
    RunManifest {
        manifest: PathBuf,
        #[arg(long, default_value = "ksa64-optimization")]
        output: PathBuf,
        #[arg(long, default_value_t = 1)]
        workers: usize,
        #[arg(long)]
        resume: bool,
        #[arg(long)]
        tui: bool,
    },
    Serve {
        id: String,
        #[arg(long, value_enum, default_value_t = EngineArg::Nsga2)]
        engine: EngineArg,
        #[arg(long, value_enum, default_value_t = PresetArg::Quick)]
        preset: PresetArg,
        #[arg(long)]
        transcript: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum EvidenceCommand {
    Inspect {
        artifact: PathBuf,
    },
    Verify {
        artifact: PathBuf,
    },
    Replay {
        artifact: PathBuf,
    },
    Debrief {
        session: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum TargetCommand {
    List,
    Show {
        id: String,
    },
    Build {
        id: String,
    },
    Verify {
        id: String,
    },
    Probe {
        id: String,
        #[arg(long)]
        live: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuditCommand {
    List,
    Run {
        phase: String,
        #[arg(long)]
        live_vice: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum DisplayArg {
    Tui,
    Summary,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum PaceArg {
    Fast,
    Realtime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum EngineArg {
    Grid,
    Nsga2,
    De,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum PresetArg {
    Quick,
    Routine,
    Accepted,
}

pub fn main_entry() -> ExitCode {
    run_from(std::env::args_os())
}

pub fn run_from<I, T>(arguments: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return ExitCode::from(2);
        }
    };
    let json_output = cli.json;
    let application = Ksa64Application::default();
    match execute_cli(&application, cli) {
        Ok(Some(outcome)) => {
            emit_outcome(&outcome, json_output);
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(error) => {
            emit_error(&error, json_output);
            ExitCode::from(error.exit_code)
        }
    }
}

pub fn execute_cli(
    application: &Ksa64Application,
    cli: Cli,
) -> Result<Option<ApplicationOutcome>, ApplicationError> {
    let json_output = cli.json;
    let Some(command) = cli.command else {
        if json_output {
            let value = quick_start_json();
            println!(
                "{}",
                serde_json::to_string_pretty(&value).expect("quick-start JSON")
            );
        } else {
            print_quick_start();
        }
        return Ok(None);
    };
    let result = match command {
        Command::Catalog { command } => {
            emit_catalog(application.catalog(), command, json_output)?;
            return Ok(None);
        }
        Command::Project { command } => project(application, command),
        Command::Mission { command } => mission(application, command),
        Command::Campaign { command } => campaign(application, command),
        Command::Optimize { command } => optimize(application, command),
        Command::Evidence { command } => evidence(application, command),
        Command::Target { command } => {
            if matches!(command, TargetCommand::List | TargetCommand::Show { .. }) {
                emit_targets(application.catalog(), command, json_output)?;
                return Ok(None);
            }
            target(application, command)
        }
        Command::Audit { command } => {
            if matches!(command, AuditCommand::List) {
                emit_audits(application.catalog(), json_output);
                return Ok(None);
            }
            audit(application, command)
        }
        Command::Capture { path } => {
            legacy_capture(&path)?;
            return Ok(None);
        }
        Command::Inspect { path } => {
            legacy_inspect(&path)?;
            return Ok(None);
        }
        Command::Phase2Capture { path } => {
            legacy_phase2_capture(&path)?;
            return Ok(None);
        }
        Command::Phase2Inspect { path } => {
            legacy_phase2_inspect(&path)?;
            return Ok(None);
        }
    }?;
    emit_outcome(&result, json_output);
    Ok(None)
}

fn project(
    application: &Ksa64Application,
    command: ProjectCommand,
) -> Result<ApplicationOutcome, ApplicationError> {
    let request = match command {
        ProjectCommand::Lint { source } => ProjectRequest::Lint {
            source: read_text(&source)?,
        },
        ProjectCommand::Compile { source, output } => ProjectRequest::Compile {
            source: read_text(&source)?,
            output,
        },
        ProjectCommand::Run { source, output } => ProjectRequest::Run {
            source: read_text(&source)?,
            output,
            scripted: false,
        },
        ProjectCommand::Script { source, output } => ProjectRequest::Run {
            source: read_text(&source)?,
            output,
            scripted: true,
        },
    };
    application.execute(ApplicationRequest::Project(request))
}

fn mission(
    application: &Ksa64Application,
    command: MissionCommand,
) -> Result<ApplicationOutcome, ApplicationError> {
    let request = match command {
        MissionCommand::Run {
            id,
            scenario,
            role,
            display,
            pace,
            output,
        } => MissionApplicationRequest::Run(MissionRequest {
            id,
            scenario,
            role,
            display: display.into(),
            pace: pace.into(),
            scripted: true,
            output,
        }),
        MissionCommand::Control {
            id,
            scenario,
            role,
            pace,
            output,
        } => MissionApplicationRequest::Control(MissionRequest {
            id,
            scenario,
            role,
            display: MissionDisplay::Tui,
            pace: pace.into(),
            scripted: false,
            output,
        }),
    };
    application.execute(ApplicationRequest::Mission(request))
}

fn campaign(
    application: &Ksa64Application,
    command: CampaignCommand,
) -> Result<ApplicationOutcome, ApplicationError> {
    match command {
        CampaignCommand::Run {
            id,
            preset,
            runs,
            workers,
            output,
        } => {
            if preset != "routine" {
                return Err(ApplicationError::unsupported(
                    "campaign.preset",
                    "current campaigns expose their accepted routine configuration; use --runs for an explicit bounded count",
                ));
            }
            application.execute(ApplicationRequest::Campaign(CampaignRequest {
                id,
                runs,
                workers,
                output,
            }))
        }
    }
}

fn optimize(
    application: &Ksa64Application,
    command: OptimizeCommand,
) -> Result<ApplicationOutcome, ApplicationError> {
    match command {
        OptimizeCommand::Compile { source, output } => {
            compile_optimization_manifest(&read_text(&source)?, &output)
        }
        OptimizeCommand::Run {
            id,
            engine,
            preset,
            workers,
            output,
            tui,
        } => application.execute(ApplicationRequest::Optimization(OptimizationRequest {
            id,
            engine: engine.into(),
            preset: preset.into(),
            workers,
            output,
            tui,
            resume: false,
        })),
        OptimizeCommand::RunManifest {
            manifest,
            output,
            workers,
            resume,
            tui,
        } => run_optimization_manifest(&manifest, &output, workers, tui, resume),
        OptimizeCommand::Serve {
            id,
            engine,
            preset,
            transcript,
        } => {
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut capture = Vec::new();
            let outcome = serve_product_optimizer(
                &id,
                engine.into(),
                preset.into(),
                stdin.lock(),
                stdout.lock(),
                &mut capture,
            )?;
            if let Some(path) = transcript {
                write_bytes(&path, &capture)?;
            }
            Ok(outcome)
        }
    }
}

fn evidence(
    application: &Ksa64Application,
    command: EvidenceCommand,
) -> Result<ApplicationOutcome, ApplicationError> {
    let request = match command {
        EvidenceCommand::Inspect { artifact } => EvidenceRequest::Inspect { artifact },
        EvidenceCommand::Verify { artifact } => EvidenceRequest::Verify { artifact },
        EvidenceCommand::Replay { artifact } => EvidenceRequest::Replay { artifact },
        EvidenceCommand::Debrief { session, output } => {
            EvidenceRequest::Debrief { session, output }
        }
    };
    application.execute(ApplicationRequest::Evidence(request))
}

fn target(
    application: &Ksa64Application,
    command: TargetCommand,
) -> Result<ApplicationOutcome, ApplicationError> {
    let request = match command {
        TargetCommand::Build { id } => TargetRequest::Build { id },
        TargetCommand::Verify { id } => TargetRequest::VerifyStored { id },
        TargetCommand::Probe { id, live } => TargetRequest::ProbeLive { id, live },
        TargetCommand::List | TargetCommand::Show { .. } => unreachable!(),
    };
    application.execute(ApplicationRequest::Target(request))
}

fn audit(
    application: &Ksa64Application,
    command: AuditCommand,
) -> Result<ApplicationOutcome, ApplicationError> {
    match command {
        AuditCommand::Run { phase, live_vice } => {
            application.execute(ApplicationRequest::Audit(AuditRequest { phase, live_vice }))
        }
        AuditCommand::List => unreachable!(),
    }
}

fn emit_catalog(
    catalog: ProductCatalog,
    command: CatalogCommand,
    json_output: bool,
) -> Result<(), ApplicationError> {
    match command {
        CatalogCommand::List { historical } => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&catalog.json(historical)).expect("catalog JSON")
                );
            } else {
                let mut experiences = catalog.experiences.iter().collect::<Vec<_>>();
                experiences.sort_by_key(|item| item.id);
                println!("KSA64 current experiences\n");
                for item in experiences {
                    println!(
                        "  {:<24} {:<13} {}",
                        item.id,
                        maturity_name(item.maturity),
                        item.name
                    );
                }
                if historical {
                    println!("\nHistorical engineering and validation");
                    let mut historical = catalog.historical.iter().collect::<Vec<_>>();
                    historical.sort_by_key(|item| phase_sort_key(item.phase));
                    for item in historical {
                        println!(
                            "  phase {:<5} {:<42} {}",
                            item.phase, item.purpose, item.audit_script
                        );
                    }
                } else {
                    println!("\nUse `ksa64 catalog list --historical` for frozen phase tools.");
                }
            }
        }
        CatalogCommand::Show { id } => {
            if let Some(item) = catalog.experience(&id) {
                if json_output {
                    println!("{}", serde_json::to_string_pretty(item).unwrap());
                } else {
                    print_experience(item);
                }
            } else if let Some(item) = catalog.historical(&id) {
                if json_output {
                    println!("{}", serde_json::to_string_pretty(item).unwrap());
                } else {
                    println!(
                        "Phase {}\n  purpose: {}\n  audit: {}",
                        item.phase, item.purpose, item.audit_script
                    );
                }
            } else {
                return Err(ApplicationError::not_found(
                    "catalog.entry-not-found",
                    format!("unknown catalog entry `{id}`"),
                ));
            }
        }
        CatalogCommand::Export { output, historical } => {
            let bytes = serde_json::to_vec_pretty(&catalog.json(historical)).expect("catalog JSON");
            write_bytes(&output, &bytes)?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "schema": "ksa64.catalog-export.v1",
                        "output": output.display().to_string(),
                        "bytes": bytes.len(),
                        "historical": historical,
                    }))
                    .unwrap()
                );
            } else {
                println!(
                    "wrote deterministic product catalog to {}",
                    output.display()
                );
            }
        }
    }
    Ok(())
}

fn emit_targets(
    catalog: ProductCatalog,
    command: TargetCommand,
    json_output: bool,
) -> Result<(), ApplicationError> {
    match command {
        TargetCommand::List => {
            let mut targets = catalog.targets.iter().collect::<Vec<_>>();
            targets.sort_by_key(|item| item.id);
            if json_output {
                println!("{}", serde_json::to_string_pretty(&targets).unwrap());
            } else {
                println!("KSA64 C64 targets\n");
                for item in targets {
                    println!(
                        "  {:<34} {:<16} {}",
                        item.id,
                        format!("{:?}", item.timing),
                        item.name
                    );
                }
                println!(
                    "\nStored verification never launches VICE; live probes require `--live`."
                );
            }
        }
        TargetCommand::Show { id } => {
            let target = catalog.target(&id).ok_or_else(|| {
                ApplicationError::not_found(
                    "catalog.target-not-found",
                    format!("unknown target `{id}`"),
                )
            })?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(target).unwrap());
            } else {
                println!(
                    "{}\n  {}\n  binary: {}\n  placement: {:?}\n  timing: {:?}\n  stock C64: {}\n  REU required: {}\n  stored evidence: {}\n  notes: {}",
                    target.id,
                    target.name,
                    target.cargo_binary,
                    target.placement,
                    target.timing,
                    target.stock_c64,
                    target.reu_required,
                    target.stored_evidence,
                    target.notes,
                );
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn emit_audits(catalog: ProductCatalog, json_output: bool) {
    let mut items = catalog.historical.iter().collect::<Vec<_>>();
    items.sort_by_key(|item| phase_sort_key(item.phase));
    if json_output {
        println!("{}", serde_json::to_string_pretty(&items).unwrap());
    } else {
        println!("KSA64 historical audits\n");
        for item in items {
            println!(
                "  phase {:<5} {:<42} {}",
                item.phase, item.purpose, item.audit_script
            );
        }
        println!("\nDefault audits do not launch VICE. Use `--live-vice` explicitly.");
    }
}

fn print_experience(item: &ExperienceDescriptor) {
    println!(
        "{}\n  {}\n  profile: {}\n  vehicle: {}\n  mission: {}\n  avionics: {}\n  maturity: {}\n  scenarios: {}\n  actions: {}\n  placements: {}\n  timing: {:?}\n  hardware: {}\n  envelope: {}\n  limitations: {}\n  historical provenance: Phase {}",
        item.id,
        item.name,
        item.profile,
        item.vehicle,
        item.mission,
        item.avionics,
        maturity_name(item.maturity),
        join_debug(item.scenarios),
        join_values(item.actions),
        join_values(item.placements),
        item.timing,
        item.hardware.notes,
        item.envelope,
        item.limitations,
        item.provenance_phase,
    );
}

fn emit_outcome(outcome: &ApplicationOutcome, json_output: bool) {
    if json_output {
        println!("{}", serde_json::to_string_pretty(outcome).unwrap());
    } else if !outcome.summary.is_empty() {
        println!("{}", outcome.summary);
        for artifact in &outcome.artifacts {
            println!("  evidence: {artifact}");
        }
    }
}

fn emit_error(error: &ApplicationError, json_output: bool) {
    if json_output {
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema": "ksa64.diagnostic.v1",
                "diagnostic": error.diagnostic,
                "exit_code": error.exit_code,
            }))
            .unwrap()
        );
    } else {
        eprintln!(
            "error [{}]: {}",
            error.diagnostic.code, error.diagnostic.message
        );
        if let Some(hint) = &error.diagnostic.hint {
            eprintln!("hint: {hint}");
        }
    }
}

fn print_quick_start() {
    println!(
        "KSA64 — deterministic aerospace simulation and mission operations\n\
         \n\
         Guided flagship\n\
           ksa64 mission control ksa-g10r.operations --scenario gnss-loss\n\
         \n\
         Nominal global flight\n\
           ksa64 mission run ksa-g10r.global --display summary --output ksa64-global\n\
         \n\
         Discover\n\
           ksa64 catalog list\n\
           ksa64 catalog show ksa-g10r.operations\n\
         \n\
         Author and replay\n\
           ksa64 project lint phase11/examples/gnss-loss.json\n\
           ksa64 evidence replay SESSION.ksb11\n\
           ksa64 evidence debrief SESSION.ksb11 --output debrief\n\
         \n\
         Commodore 64 targets\n\
           ksa64 target list\n\
           ksa64 target verify c64.ksa-g10r.reference-ops\n\
         \n\
         Historical validation\n\
           ksa64 audit list\n\
         \n\
         Nothing above launches hardware or VICE implicitly. Run `ksa64 --help` for all commands."
    );
}

fn quick_start_json() -> serde_json::Value {
    json!({
        "schema": "ksa64.quick-start.v1",
        "flagship": ["mission", "control", "ksa-g10r.operations", "--scenario", "gnss-loss"],
        "nominal_global": ["mission", "run", "ksa-g10r.global", "--display", "summary"],
        "catalog": ["catalog", "list"],
        "projects": ["project", "lint", "phase11/examples/gnss-loss.json"],
        "targets": ["target", "list"],
        "audits": ["audit", "list"],
        "mutates_external_state": false,
        "launches_vice": false,
    })
}

fn legacy_inspect(path: &Path) -> Result<(), ApplicationError> {
    let scenario = parse_scenario_image(LEGACY_SCENARIO).map_err(|error| {
        ApplicationError::integrity(
            "legacy.scenario",
            format!("built-in scenario is invalid: {error:?}"),
        )
    })?;
    let stream = fs::read(path).map_err(io_error("legacy.read"))?;
    let inspection = inspect_stream(&stream, &scenario).map_err(|error| {
        ApplicationError::integrity(
            "legacy.stream",
            format!(
                "{} is not a valid mission stream: {error:?}",
                path.display()
            ),
        )
    })?;
    print!("{}", format_inspection(inspection));
    Ok(())
}

fn legacy_capture(path: &Path) -> Result<(), ApplicationError> {
    let scenario = parse_scenario_image(LEGACY_SCENARIO).map_err(|error| {
        ApplicationError::integrity(
            "legacy.scenario",
            format!("built-in scenario is invalid: {error:?}"),
        )
    })?;
    let file = File::create(path).map_err(io_error("legacy.create"))?;
    let mut writer = BufWriter::new(file);
    let summary = capture_mission(&scenario, &mut writer).map_err(|error| {
        ApplicationError::execution(
            "legacy.capture",
            format!("mission capture failed: {error:?}"),
        )
    })?;
    writer.flush().map_err(io_error("legacy.flush"))?;
    eprintln!(
        "captured {} frames through step {} to {}",
        summary.frames_written(),
        summary.mission().completed_steps(),
        path.display()
    );
    legacy_inspect(path)
}

fn legacy_phase2_inspect(path: &Path) -> Result<(), ApplicationError> {
    let scenario = parse_phase2_scenario(LEGACY_PHASE2_SCENARIO).map_err(|error| {
        ApplicationError::integrity(
            "legacy.phase2-scenario",
            format!("built-in Phase 2 scenario is invalid: {error:?}"),
        )
    })?;
    let stream = fs::read(path).map_err(io_error("legacy.phase2-read"))?;
    let inspection = inspect_phase2_stream(&stream, &scenario).map_err(|error| {
        ApplicationError::integrity(
            "legacy.phase2-stream",
            format!(
                "{} is not a valid Phase 2 stream: {error:?}",
                path.display()
            ),
        )
    })?;
    print!("{}", format_phase2_inspection(inspection));
    Ok(())
}

fn legacy_phase2_capture(path: &Path) -> Result<(), ApplicationError> {
    let scenario = parse_phase2_scenario(LEGACY_PHASE2_SCENARIO).map_err(|error| {
        ApplicationError::integrity(
            "legacy.phase2-scenario",
            format!("built-in Phase 2 scenario is invalid: {error:?}"),
        )
    })?;
    let file = File::create(path).map_err(io_error("legacy.phase2-create"))?;
    let mut writer = BufWriter::new(file);
    let summary = capture_phase2_mission(&scenario, &mut writer).map_err(|error| {
        ApplicationError::execution(
            "legacy.phase2-capture",
            format!("Phase 2 mission capture failed: {error:?}"),
        )
    })?;
    writer.flush().map_err(io_error("legacy.phase2-flush"))?;
    eprintln!(
        "captured {} Phase 2 frames through step {} to {}",
        summary.frames_written(),
        summary.mission().truth().step(),
        path.display()
    );
    legacy_phase2_inspect(path)
}

fn read_text(path: &Path) -> Result<String, ApplicationError> {
    fs::read_to_string(path).map_err(io_error("filesystem.read"))
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), ApplicationError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(io_error("filesystem.create-directory"))?;
    }
    fs::write(path, bytes).map_err(io_error("filesystem.write"))
}

fn io_error(operation: &'static str) -> impl FnOnce(std::io::Error) -> ApplicationError {
    move |error| {
        ApplicationError::execution("application.io", format!("{operation} failed: {error}"))
    }
}

fn maturity_name(maturity: EvidenceMaturity) -> &'static str {
    match maturity {
        EvidenceMaturity::Accepted => "accepted",
        EvidenceMaturity::Qualified => "qualified",
        EvidenceMaturity::Experimental => "experimental",
    }
}

fn phase_sort_key(phase: &str) -> u32 {
    match phase.split_once('.') {
        Some((major, minor)) => {
            major.parse::<u32>().unwrap_or(999) * 10 + minor.parse::<u32>().unwrap_or(9)
        }
        None => phase.parse::<u32>().unwrap_or(999) * 10,
    }
}

fn join_debug(values: &[&str]) -> String {
    if values.is_empty() {
        "none".into()
    } else {
        values.join(", ")
    }
}

fn join_values<T: std::fmt::Debug>(values: &[T]) -> String {
    if values.is_empty() {
        "none".into()
    } else {
        values
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl From<DisplayArg> for MissionDisplay {
    fn from(value: DisplayArg) -> Self {
        match value {
            DisplayArg::Tui => Self::Tui,
            DisplayArg::Summary => Self::Summary,
            DisplayArg::None => Self::None,
        }
    }
}

impl From<PaceArg> for MissionPace {
    fn from(value: PaceArg) -> Self {
        match value {
            PaceArg::Fast => Self::Fast,
            PaceArg::Realtime => Self::Realtime,
        }
    }
}

impl From<EngineArg> for SearchEngineId {
    fn from(value: EngineArg) -> Self {
        match value {
            EngineArg::Grid => Self::GridV1,
            EngineArg::Nsga2 => Self::Nsga2V1,
            EngineArg::De => Self::DifferentialEvolutionV1,
        }
    }
}

impl From<PresetArg> for SearchPresetId {
    fn from(value: PresetArg) -> Self {
        match value {
            PresetArg::Quick => Self::Quick,
            PresetArg::Routine => Self::Routine,
            PresetArg::Accepted => Self::AcceptedBalanced,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_is_nonmutating_quick_start() {
        let parsed = Cli::try_parse_from(["ksa64"]).unwrap();
        assert!(parsed.command.is_none());
        let value = quick_start_json();
        assert_eq!(value["launches_vice"], false);
        assert_eq!(value["mutates_external_state"], false);
    }

    #[test]
    fn flagship_command_parses_with_domain_identity() {
        let parsed = Cli::try_parse_from([
            "ksa64",
            "mission",
            "control",
            "ksa-g10r.operations",
            "--scenario",
            "gnss-loss",
        ])
        .unwrap();
        assert!(matches!(
            parsed.command,
            Some(Command::Mission {
                command: MissionCommand::Control { id, .. }
            }) if id == "ksa-g10r.operations"
        ));
    }

    #[test]
    fn live_target_flag_is_never_implicit() {
        let parsed =
            Cli::try_parse_from(["ksa64", "target", "probe", "c64.ksa-g10r.global-flight"])
                .unwrap();
        assert!(matches!(
            parsed.command,
            Some(Command::Target {
                command: TargetCommand::Probe { live: false, .. }
            })
        ));
    }

    #[test]
    fn catalog_json_is_deterministic() {
        let catalog = ProductCatalog::accepted();
        let first = serde_json::to_vec_pretty(&catalog.json(true)).unwrap();
        let second = serde_json::to_vec_pretty(&catalog.json(true)).unwrap();
        assert_eq!(first, second);
    }
}
