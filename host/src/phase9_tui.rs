//! Seven-page passive optimization workbench presentation.
use crate::phase9_search::SearchResult;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs, Wrap};
use ratatui::{Frame, Terminal};
use std::io;
use std::time::Duration;
const PAGES: [&str; 7] = [
    "F1 STATUS",
    "F2 PARETO",
    "F3 CANDIDATES",
    "F4 SENSITIVITY",
    "F5 CONSTRAINTS",
    "F6 INSPECT",
    "F7 ARCHIVE",
];
#[derive(Debug)]
pub enum OptimizationTuiError {
    Io(io::Error),
}
impl From<io::Error> for OptimizationTuiError {
    fn from(v: io::Error) -> Self {
        Self::Io(v)
    }
}
pub fn run_optimization_tui(result: &SearchResult) -> Result<(), OptimizationTuiError> {
    enable_raw_mode()?;
    let mut err = io::stderr();
    execute!(err, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(err);
    let mut terminal = Terminal::new(backend)?;
    let mut page = 0usize;
    loop {
        terminal.draw(|f| draw(f, result, page))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match k.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::F(n) if (1..=7).contains(&n) => page = n as usize - 1,
                        KeyCode::Right | KeyCode::Tab => page = (page + 1) % 7,
                        KeyCode::Left => page = (page + 6) % 7,
                        _ => {}
                    }
                }
            }
        }
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
fn draw(f: &mut Frame<'_>, result: &SearchResult, page: usize) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());
    let titles = PAGES.iter().map(|x| Line::from(*x)).collect::<Vec<_>>();
    f.render_widget(
        Tabs::new(titles)
            .select(page)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" KSA64 // PHASE 9 OPTIMIZATION "),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
        chunks[0],
    );
    let text = page_lines(result, page);
    f.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(PAGES[page]))
            .wrap(Wrap { trim: false }),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new("F1–F7 pages  ←/→ navigate  Q quit  • presentation is passive"),
        chunks[2],
    );
}
fn page_lines(result: &SearchResult, page: usize) -> Vec<Line<'static>> {
    let last = result.generations.last();
    let mut out = Vec::new();
    match page {
        0 => {
            out.push(kv("Manifest", format!("{:08X}", result.manifest_identity)));
            out.push(kv(
                "Generation boundaries",
                result.generations.len().to_string(),
            ));
            out.push(kv("Exact evaluations", result.evaluations.to_string()));
            out.push(kv("Cache reuse", result.cache_hits.to_string()));
            out.push(kv(
                "Pareto candidates",
                result.pareto_indices.len().to_string(),
            ));
            out.push(kv("64-case finalists", result.finalists.len().to_string()))
        }
        1 => {
            out.push(Line::from(
                "Candidate       O0            O1            O2     Feasible",
            ));
            if let Some(g) = last {
                for i in &result.pareto_indices {
                    let a = g.aggregates[*i];
                    out.push(Line::from(format!(
                        "{:08X}  {:>12}  {:>12}  {:>12}  {}",
                        a.candidate_identity,
                        a.objectives[0],
                        a.objectives[1],
                        a.objectives[2],
                        a.feasible
                    )))
                }
            }
        }
        2 => {
            out.push(Line::from(
                "Parallel-coordinate raw objectives (compact listing)",
            ));
            if let Some(g) = last {
                for a in g.aggregates.iter().take(24) {
                    out.push(Line::from(format!(
                        "{:08X} | {:>10} | {:>10} | {:>10} | {:>10}",
                        a.candidate_identity,
                        a.objectives[0],
                        a.objectives[1],
                        a.objectives[2],
                        a.objectives[3]
                    )))
                }
            }
        }
        3 => {
            out.push(Line::from("One-quantum local sensitivities and accepted grid heatmaps are exported with the report."));
            out.push(Line::from(
                "This page is intentionally model-derived evidence, not causal flight evidence.",
            ));
        }
        4 => {
            out.push(Line::from(
                "Candidate       Fatal  Violations  Normalized sum",
            ));
            if let Some(g) = last {
                for a in g.aggregates.iter().filter(|x| !x.feasible).take(24) {
                    out.push(Line::from(format!(
                        "{:08X}      {:>2}       {:>2}      {:>14}",
                        a.candidate_identity,
                        a.fatal_class,
                        a.violated_constraints,
                        a.normalized_violation
                    )))
                }
            }
        }
        5 => {
            out.push(Line::from("Terminal finalist evidence"));
            for f in result.finalists.iter().take(16) {
                out.push(Line::from(format!(
                    "{:08X} tier {:>2} cases {:>2} objectives {:?}",
                    f.aggregate.candidate_identity,
                    f.aggregate.uncertainty_tier,
                    f.aggregate.case_count,
                    &f.aggregate.objectives[..f.aggregate.objective_count as usize]
                )))
            }
        }
        _ => {
            for g in &result.generations {
                out.push(Line::from(format!(
                    "Generation {:>3}: {:>5} candidates  CRC {:08X}",
                    g.index,
                    g.candidates.len(),
                    g.crc32
                )))
            }
            out.push(kv(
                "Resume contract",
                "only complete generation segments".into(),
            ));
            out.push(kv(
                "Worker scheduling",
                "excluded from experiment identity".into(),
            ))
        }
    }
    out
}
fn kv(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<24}"), Style::default().fg(Color::Cyan)),
        Span::raw(value),
    ])
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase9_search::SearchResult;
    #[test]
    fn every_page_renders_without_truth_mutation() {
        let r = SearchResult {
            manifest_identity: 1,
            generations: vec![],
            pareto_indices: vec![],
            finalists: vec![],
            cache_hits: 0,
            evaluations: 0,
        };
        for p in 0..7 {
            assert!(!page_lines(&r, p).is_empty())
        }
    }
}
