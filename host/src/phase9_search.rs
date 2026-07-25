//! Deterministic Phase 9 search engines and feasibility-first ordering.
use crate::phase9::{baseline_vector, CandidateEvaluation};
use ksa64_core::phase9_contract::{
    CandidateAggregate, DesignVector, Direction, SearchEngineId, SearchManifest, VariableKind,
};
use ksa64_sim::phase4::campaign::keyed_word_raw;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::mpsc;
use std::thread;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchGeneration {
    pub index: u16,
    pub candidates: Vec<DesignVector>,
    pub aggregates: Vec<CandidateAggregate>,
    pub crc32: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub manifest_identity: u32,
    pub generations: Vec<SearchGeneration>,
    pub pareto_indices: Vec<usize>,
    pub finalists: Vec<CandidateEvaluation>,
    pub cache_hits: u32,
    pub evaluations: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchError {
    Configuration,
    Evaluation,
    CandidateLimit,
}

pub trait CandidateEvaluator {
    fn evaluate(
        &self,
        candidate: &DesignVector,
        tier: u8,
    ) -> Result<CandidateEvaluation, SearchError>;
}
impl<F> CandidateEvaluator for F
where
    F: Fn(&DesignVector, u8) -> Result<CandidateEvaluation, SearchError>,
{
    fn evaluate(&self, c: &DesignVector, t: u8) -> Result<CandidateEvaluation, SearchError> {
        self(c, t)
    }
}

pub fn feasibility_cmp(a: &CandidateAggregate, b: &CandidateAggregate) -> Ordering {
    match (a.feasible, b.feasible) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => (
            a.fatal_class,
            a.violated_constraints,
            a.normalized_violation,
            a.candidate_identity,
        )
            .cmp(&(
                b.fatal_class,
                b.violated_constraints,
                b.normalized_violation,
                b.candidate_identity,
            )),
        (true, true) => a.candidate_identity.cmp(&b.candidate_identity),
    }
}
pub fn dominates(m: &SearchManifest, a: &CandidateAggregate, b: &CandidateAggregate) -> bool {
    if a.feasible && !b.feasible {
        return true;
    }
    if !a.feasible {
        return false;
    }
    if !b.feasible {
        return true;
    }
    let mut better = false;
    for i in 0..m.objective_count as usize {
        let av = a.objectives[i];
        let bv = b.objectives[i];
        match m.objectives[i].direction {
            Direction::Minimize => {
                if av > bv {
                    return false;
                }
                if av < bv {
                    better = true
                }
            }
            Direction::Maximize => {
                if av < bv {
                    return false;
                }
                if av > bv {
                    better = true
                }
            }
        }
    }
    better
}
pub fn pareto_front(m: &SearchManifest, values: &[CandidateAggregate]) -> Vec<usize> {
    let mut out = Vec::new();
    for i in 0..values.len() {
        if !values[i].feasible {
            continue;
        }
        if !(0..values.len()).any(|j| j != i && dominates(m, &values[j], &values[i])) {
            out.push(i)
        }
    }
    out.sort_by_key(|i| values[*i].candidate_identity);
    out
}

fn quantized(spec: ksa64_core::phase9_contract::VariableSpec, numer: u64, denom: u64) -> i32 {
    if denom == 0 {
        return spec.minimum;
    }
    let steps =
        ((i64::from(spec.maximum) - i64::from(spec.minimum)) / i64::from(spec.quantum)) as u64;
    let k = ((u128::from(numer) * u128::from(steps) + u128::from(denom) / 2) / u128::from(denom))
        as i64;
    (i64::from(spec.minimum) + k * i64::from(spec.quantum)) as i32
}
fn from_values(m: &SearchManifest, values: [i32; 32]) -> DesignVector {
    DesignVector {
        identity: 0,
        manifest_identity: m.identity,
        value_count: m.variable_count,
        values,
        materialized_ids: [0; 4],
    }
    .seal()
    .unwrap()
}
pub(crate) fn generation_fingerprint(
    index: u16,
    c: &[DesignVector],
    a: &[CandidateAggregate],
) -> u32 {
    let mut bytes = Vec::with_capacity(2 + c.len() * 8);
    bytes.extend_from_slice(&index.to_le_bytes());
    for (x, y) in c.iter().zip(a) {
        bytes.extend_from_slice(&x.identity.to_le_bytes());
        bytes.extend_from_slice(&y.identity.to_le_bytes())
    }
    ksa64_interface::crc32_ieee(&bytes)
}

struct EvalCache<'a, E> {
    evaluator: &'a E,
    values: BTreeMap<(u32, u8), CandidateEvaluation>,
    hits: u32,
    calls: u32,
}
impl<'a, E: CandidateEvaluator + Sync> EvalCache<'a, E> {
    fn new(e: &'a E) -> Self {
        Self {
            evaluator: e,
            values: BTreeMap::new(),
            hits: 0,
            calls: 0,
        }
    }
    fn prefetch(
        &mut self,
        candidates: &[DesignVector],
        tier: u8,
        workers: usize,
    ) -> Result<(), SearchError> {
        let mut missing = BTreeMap::new();
        for candidate in candidates {
            if !self.values.contains_key(&(candidate.identity, tier)) {
                missing.entry(candidate.identity).or_insert(*candidate);
            } else {
                self.hits = self.hits.saturating_add(1);
            }
        }
        let jobs: Vec<DesignVector> = missing.into_values().collect();
        if jobs.is_empty() {
            return Ok(());
        }
        let worker_count = workers.max(1).min(jobs.len());
        let mut ordered: BTreeMap<u32, CandidateEvaluation> = BTreeMap::new();
        if worker_count == 1 {
            for c in &jobs {
                ordered.insert(c.identity, self.evaluator.evaluate(c, tier)?);
            }
        } else {
            let (tx, rx) = mpsc::channel();
            thread::scope(|scope| {
                for worker in 0..worker_count {
                    let tx = tx.clone();
                    let eval = self.evaluator;
                    let jobs = &jobs;
                    scope.spawn(move || {
                        for index in (worker..jobs.len()).step_by(worker_count) {
                            let c = jobs[index];
                            let _ = tx.send((c.identity, eval.evaluate(&c, tier)));
                        }
                    });
                }
            });
            drop(tx);
            for _ in 0..jobs.len() {
                let (id, result) = rx.recv().map_err(|_| SearchError::Evaluation)?;
                ordered.insert(id, result?);
            }
        }
        self.calls = self.calls.saturating_add(jobs.len() as u32);
        for (id, value) in ordered {
            self.values.insert((id, tier), value);
        }
        Ok(())
    }
    fn cached(&self, c: &DesignVector, tier: u8) -> Result<CandidateEvaluation, SearchError> {
        self.values
            .get(&(c.identity, tier))
            .cloned()
            .ok_or(SearchError::Evaluation)
    }
}

pub fn grid_candidates(
    m: &SearchManifest,
    axes: &[usize],
) -> Result<Vec<DesignVector>, SearchError> {
    if axes.is_empty() || axes.len() > 2 || axes.iter().any(|i| *i >= m.variable_count as usize) {
        return Err(SearchError::Configuration);
    }
    let points = usize::from(m.budgets.grid_points.max(1));
    let count = points.pow(axes.len() as u32);
    if count as u32 > m.budgets.max_candidates {
        return Err(SearchError::CandidateLimit);
    }
    let baseline = baseline_vector(m);
    let mut out = Vec::with_capacity(count);
    for n in 0..count {
        let mut values = baseline.values;
        let mut rem = n;
        for axis in axes.iter().rev() {
            let digit = rem % points;
            rem /= points;
            values[*axis] = quantized(m.variables[*axis], digit as u64, (points - 1).max(1) as u64)
        }
        out.push(from_values(m, values))
    }
    Ok(out)
}

fn initial_population(m: &SearchManifest) -> Vec<DesignVector> {
    let n = usize::from(m.budgets.population.max(1));
    let baseline = baseline_vector(m);
    let mut out = Vec::with_capacity(n);
    out.push(baseline);
    for individual in 1..n {
        let mut values = baseline.values;
        for v in 0..m.variable_count as usize {
            let word = keyed_word_raw(m.master_seed, individual as u32, v as u8, 0, 0);
            let stratum = ((individual + v * 17) % n) as u64;
            let numer = stratum * (u64::from(u32::MAX) + 1) + u64::from(word);
            values[v] = quantized(
                m.variables[v],
                numer,
                n as u64 * (u64::from(u32::MAX) + 1) - 1,
            )
        }
        out.push(from_values(m, values))
    }
    out
}
fn tournament(m: &SearchManifest, a: &CandidateAggregate, b: &CandidateAggregate) -> bool {
    if dominates(m, a, b) {
        true
    } else if dominates(m, b, a) {
        false
    } else if a.feasible != b.feasible {
        a.feasible
    } else if !a.feasible {
        feasibility_cmp(a, b) != Ordering::Greater
    } else {
        a.candidate_identity <= b.candidate_identity
    }
}
fn offspring(
    m: &SearchManifest,
    g: u16,
    pop: &[DesignVector],
    agg: &[CandidateAggregate],
) -> Vec<DesignVector> {
    let n = pop.len();
    let mut out = Vec::with_capacity(n);
    for child in 0..n {
        let draw = |d: u8| keyed_word_raw(m.master_seed, g as u32, child as u8, 11, d);
        let a = (draw(0) as usize) % n;
        let b = (draw(1) as usize) % n;
        let p1 = if tournament(m, &agg[a], &agg[b]) {
            a
        } else {
            b
        };
        let c = (draw(2) as usize) % n;
        let d = (draw(3) as usize) % n;
        let p2 = if tournament(m, &agg[c], &agg[d]) {
            c
        } else {
            d
        };
        let mut values = pop[p1].values;
        for v in 0..m.variable_count as usize {
            let spec = m.variables[v];
            let x = pop[p1].values[v];
            let y = pop[p2].values[v];
            let cross = keyed_word_raw(m.master_seed, g as u32, child as u8, v as u8, 4);
            values[v] = match spec.kind {
                VariableKind::Boolean | VariableKind::Catalogue | VariableKind::Ordinal => {
                    if cross & 1 == 0 {
                        x
                    } else {
                        y
                    }
                }
                _ => {
                    let alpha = (cross & 0xffff) as i64;
                    let mixed =
                        (i64::from(x) * (65_535 - alpha) + i64::from(y) * alpha + 32_767) / 65_535;
                    mixed.clamp(i64::from(spec.minimum), i64::from(spec.maximum)) as i32
                }
            };
            let mutation = keyed_word_raw(m.master_seed, g as u32, child as u8, v as u8, 5);
            if mutation % 8 == 0 {
                let delta = match (mutation >> 8) % 3 {
                    0 => -1,
                    1 => 1,
                    _ => ((mutation >> 16) % 5) as i64 - 2,
                };
                values[v] = (i64::from(values[v]) + delta * i64::from(spec.quantum))
                    .clamp(i64::from(spec.minimum), i64::from(spec.maximum))
                    as i32
            }
            values[v] = quantized(
                spec,
                (i64::from(values[v]) - i64::from(spec.minimum)) as u64,
                (i64::from(spec.maximum) - i64::from(spec.minimum)).max(1) as u64,
            )
        }
        out.push(from_values(m, values))
    }
    out
}

fn nondominated_ranks(m: &SearchManifest, a: &[CandidateAggregate]) -> Vec<u16> {
    let mut rank = vec![u16::MAX; a.len()];
    let mut remaining: Vec<usize> = (0..a.len()).collect();
    let mut r = 0u16;
    while !remaining.is_empty() {
        let front: Vec<usize> = remaining
            .iter()
            .copied()
            .filter(|i| {
                !remaining
                    .iter()
                    .any(|j| i != j && dominates(m, &a[*j], &a[*i]))
            })
            .collect();
        if front.is_empty() {
            for i in remaining {
                rank[i] = r
            }
            break;
        }
        for i in &front {
            rank[*i] = r
        }
        remaining.retain(|i| !front.contains(i));
        r = r.saturating_add(1)
    }
    rank
}
fn crowding(
    m: &SearchManifest,
    indices: &[usize],
    a: &[CandidateAggregate],
) -> BTreeMap<usize, i128> {
    let mut score: BTreeMap<usize, i128> = indices.iter().map(|i| (*i, 0)).collect();
    if indices.len() <= 2 {
        for i in indices {
            score.insert(*i, i128::MAX / 4);
        }
        return score;
    }
    for o in 0..m.objective_count as usize {
        let mut sorted = indices.to_vec();
        sorted.sort_by_key(|i| (a[*i].objectives[o], a[*i].candidate_identity));
        let min = a[sorted[0]].objectives[o];
        let max = a[*sorted.last().unwrap()].objectives[o];
        score.insert(sorted[0], i128::MAX / 4);
        score.insert(*sorted.last().unwrap(), i128::MAX / 4);
        let range = (i128::from(max) - i128::from(min)).abs().max(1);
        for w in sorted.windows(3) {
            let mid = w[1];
            if score[&mid] < i128::MAX / 8 {
                let delta =
                    (i128::from(a[w[2]].objectives[o]) - i128::from(a[w[0]].objectives[o])).abs();
                *score.get_mut(&mid).unwrap() += delta * 1_000_000_000 / range
            }
        }
    }
    score
}
fn select_nsga(
    m: &SearchManifest,
    candidates: Vec<DesignVector>,
    aggregates: Vec<CandidateAggregate>,
    n: usize,
) -> (Vec<DesignVector>, Vec<CandidateAggregate>) {
    let ranks = nondominated_ranks(m, &aggregates);
    let mut indices: Vec<usize> = (0..candidates.len()).collect();
    let mut scores = BTreeMap::new();
    let max = *ranks.iter().max().unwrap_or(&0);
    for r in 0..=max {
        let f: Vec<usize> = indices.iter().copied().filter(|i| ranks[*i] == r).collect();
        scores.extend(crowding(m, &f, &aggregates))
    }
    indices.sort_by(|a, b| {
        ranks[*a]
            .cmp(&ranks[*b])
            .then_with(|| scores.get(b).unwrap_or(&0).cmp(scores.get(a).unwrap_or(&0)))
            .then_with(|| {
                aggregates[*a]
                    .candidate_identity
                    .cmp(&aggregates[*b].candidate_identity)
            })
    });
    indices.truncate(n);
    let c = indices.iter().map(|i| candidates[*i]).collect();
    let a = indices.iter().map(|i| aggregates[*i]).collect();
    (c, a)
}

fn evaluate_generation<E: CandidateEvaluator + Sync>(
    cache: &mut EvalCache<E>,
    index: u16,
    candidates: Vec<DesignVector>,
    workers: usize,
) -> Result<SearchGeneration, SearchError> {
    cache.prefetch(&candidates, 8, workers)?;
    let mut aggregates = Vec::with_capacity(candidates.len());
    for c in &candidates {
        aggregates.push(cache.cached(c, 8)?.aggregate)
    }
    let crc = generation_fingerprint(index, &candidates, &aggregates);
    Ok(SearchGeneration {
        index,
        candidates,
        aggregates,
        crc32: crc,
    })
}

fn run_nsga<E: CandidateEvaluator + Sync>(
    m: &SearchManifest,
    cache: &mut EvalCache<E>,
    workers: usize,
) -> Result<Vec<SearchGeneration>, SearchError> {
    let mut generations = Vec::new();
    let mut current = evaluate_generation(cache, 0, initial_population(m), workers)?;
    generations.push(current.clone());
    for g in 1..=m.budgets.generations {
        let children = offspring(m, g, &current.candidates, &current.aggregates);
        let child = evaluate_generation(cache, g, children, workers)?;
        let mut all_c = current.candidates.clone();
        all_c.extend_from_slice(&child.candidates);
        let mut all_a = current.aggregates.clone();
        all_a.extend_from_slice(&child.aggregates);
        let (c, a) = select_nsga(m, all_c, all_a, m.budgets.population as usize);
        let crc = generation_fingerprint(g, &c, &a);
        current = SearchGeneration {
            index: g,
            candidates: c,
            aggregates: a,
            crc32: crc,
        };
        generations.push(current.clone())
    }
    Ok(generations)
}

fn reflect(mut v: i64, min: i64, max: i64) -> i64 {
    if min == max {
        return min;
    }
    while v < min || v > max {
        if v < min {
            v = min + (min - v)
        }
        if v > max {
            v = max - (v - max)
        }
    }
    v
}
fn run_de<E: CandidateEvaluator + Sync>(
    m: &SearchManifest,
    cache: &mut EvalCache<E>,
    workers: usize,
) -> Result<Vec<SearchGeneration>, SearchError> {
    let mut generations = Vec::new();
    let mut current = evaluate_generation(cache, 0, initial_population(m), workers)?;
    generations.push(current.clone());
    let n = current.candidates.len();
    if n < 4 {
        return Err(SearchError::Configuration);
    }
    for g in 1..=m.budgets.generations {
        let mut trials = Vec::with_capacity(n);
        for target in 0..n {
            let pick =
                |d: u8| (keyed_word_raw(m.master_seed, g as u32, target as u8, 41, d) as usize) % n;
            let mut r1 = pick(0);
            while r1 == target {
                r1 = (r1 + 1) % n
            }
            let mut r2 = pick(1);
            while r2 == target || r2 == r1 {
                r2 = (r2 + 1) % n
            }
            let mut r3 = pick(2);
            while r3 == target || r3 == r1 || r3 == r2 {
                r3 = (r3 + 1) % n
            }
            let forced = (pick(3)) % (m.variable_count as usize);
            let mut values = current.candidates[target].values;
            for v in 0..m.variable_count as usize {
                let spec = m.variables[v];
                if matches!(spec.kind, VariableKind::Catalogue) {
                    continue;
                }
                let cross =
                    keyed_word_raw(m.master_seed, g as u32, target as u8, v as u8, 4) & 0xffff;
                if v == forced || cross <= 58_981 {
                    let donor = i64::from(current.candidates[r1].values[v])
                        + ((i64::from(current.candidates[r2].values[v])
                            - i64::from(current.candidates[r3].values[v]))
                            * 3
                            / 4);
                    let bounded = reflect(donor, i64::from(spec.minimum), i64::from(spec.maximum));
                    let steps = (bounded - i64::from(spec.minimum) + i64::from(spec.quantum) / 2)
                        / i64::from(spec.quantum);
                    values[v] = (i64::from(spec.minimum) + steps * i64::from(spec.quantum))
                        .clamp(i64::from(spec.minimum), i64::from(spec.maximum))
                        as i32
                }
            }
            trials.push(from_values(m, values))
        }
        let trial = evaluate_generation(cache, g, trials, workers)?;
        let mut next_c = Vec::with_capacity(n);
        let mut next_a = Vec::with_capacity(n);
        for i in 0..n {
            let choose_trial = if dominates(m, &trial.aggregates[i], &current.aggregates[i]) {
                true
            } else if dominates(m, &current.aggregates[i], &trial.aggregates[i]) {
                false
            } else if trial.aggregates[i].feasible != current.aggregates[i].feasible {
                trial.aggregates[i].feasible
            } else {
                lex_objectives(m, &trial.aggregates[i], &current.aggregates[i]) != Ordering::Greater
            };
            if choose_trial {
                next_c.push(trial.candidates[i]);
                next_a.push(trial.aggregates[i])
            } else {
                next_c.push(current.candidates[i]);
                next_a.push(current.aggregates[i])
            }
        }
        let crc = generation_fingerprint(g, &next_c, &next_a);
        current = SearchGeneration {
            index: g,
            candidates: next_c,
            aggregates: next_a,
            crc32: crc,
        };
        generations.push(current.clone())
    }
    Ok(generations)
}
fn lex_objectives(m: &SearchManifest, a: &CandidateAggregate, b: &CandidateAggregate) -> Ordering {
    if a.feasible != b.feasible {
        return if a.feasible {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    if !a.feasible {
        return feasibility_cmp(a, b);
    }
    for i in 0..m.objective_count as usize {
        let cmp = match m.objectives[i].direction {
            Direction::Minimize => a.objectives[i].cmp(&b.objectives[i]),
            Direction::Maximize => b.objectives[i].cmp(&a.objectives[i]),
        };
        if cmp != Ordering::Equal {
            return cmp;
        }
    }
    a.candidate_identity.cmp(&b.candidate_identity)
}

pub fn run_search<E: CandidateEvaluator + Sync>(
    m: &SearchManifest,
    evaluator: &E,
    grid_axes: &[usize],
) -> Result<SearchResult, SearchError> {
    run_search_with_workers(m, evaluator, grid_axes, 1)
}

pub fn run_search_with_workers<E: CandidateEvaluator + Sync>(
    m: &SearchManifest,
    evaluator: &E,
    grid_axes: &[usize],
    workers: usize,
) -> Result<SearchResult, SearchError> {
    m.validate().map_err(|_| SearchError::Configuration)?;
    let mut cache = EvalCache::new(evaluator);
    let generations = match m.engine {
        SearchEngineId::GridV1 => vec![evaluate_generation(
            &mut cache,
            0,
            grid_candidates(m, grid_axes)?,
            workers,
        )?],
        SearchEngineId::Nsga2V1 => run_nsga(m, &mut cache, workers)?,
        SearchEngineId::DifferentialEvolutionV1 => run_de(m, &mut cache, workers)?,
    };
    let last = generations.last().ok_or(SearchError::Configuration)?;
    let pareto = pareto_front(m, &last.aggregates);
    let mut terminal: Vec<usize> = if pareto.is_empty() {
        let mut x: Vec<usize> = (0..last.candidates.len()).collect();
        x.sort_by(|a, b| feasibility_cmp(&last.aggregates[*a], &last.aggregates[*b]));
        x
    } else {
        pareto.clone()
    };
    terminal.truncate(m.budgets.finalists.min(terminal.len() as u16) as usize);
    let finalist_candidates: Vec<DesignVector> =
        terminal.iter().map(|i| last.candidates[*i]).collect();
    cache.prefetch(&finalist_candidates, 64, workers)?;
    let mut finalists = Vec::new();
    for candidate in &finalist_candidates {
        finalists.push(cache.cached(candidate, 64)?)
    }
    Ok(SearchResult {
        manifest_identity: m.identity,
        generations,
        pareto_indices: pareto,
        finalists,
        cache_hits: cache.hits,
        evaluations: cache.calls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase9::{built_in_manifest, StudyId};
    fn synthetic(m: SearchManifest) -> impl CandidateEvaluator {
        move |d: &DesignVector, tier: u8| {
            let x = d.values[0] / m.variables[0].quantum as i32;
            let y = d.values[1] / m.variables[1].quantum as i32;
            let mut objectives = [0; 8];
            objectives[0] = x * x + y * y;
            objectives[1] = (x - 4) * (x - 4) + (y - 4) * (y - 4);
            let a = CandidateAggregate {
                identity: 0,
                manifest_identity: m.identity,
                candidate_identity: d.identity,
                uncertainty_tier: tier,
                case_count: tier,
                fatal_class: 0,
                violated_constraints: 0,
                feasible: true,
                case_crc: d.identity ^ u32::from(tier),
                normalized_violation: 0,
                objective_count: 2,
                constraint_count: 0,
                objectives,
                constraint_values: [0; 16],
            }
            .seal();
            Ok(CandidateEvaluation {
                aggregate: a,
                cases: Vec::new(),
            })
        }
    }
    #[test]
    fn analytic_pareto_fixture_is_exact() {
        let mut m = built_in_manifest(
            StudyId::GimbalControl,
            SearchEngineId::GridV1,
            ksa64_core::phase9_contract::SearchPresetId::Quick,
        );
        m.variable_count = 2;
        m.objective_count = 2;
        m.constraint_count = 0;
        m.variables[0].minimum = 0;
        m.variables[0].maximum = 4;
        m.variables[0].quantum = 1;
        m.variables[1] = m.variables[0];
        m.variables[1].id = 999;
        m.budgets.grid_points = 5;
        m = m.seal().unwrap();
        let e = synthetic(m);
        let r = run_search(&m, &e, &[0, 1]).unwrap();
        assert!(!r.pareto_indices.is_empty());
        assert_eq!(r.generations[0].candidates.len(), 25)
    }
    #[test]
    fn nsga_and_de_are_byte_repeatable() {
        for engine in [
            SearchEngineId::Nsga2V1,
            SearchEngineId::DifferentialEvolutionV1,
        ] {
            let mut m = built_in_manifest(
                StudyId::GimbalControl,
                engine,
                ksa64_core::phase9_contract::SearchPresetId::Quick,
            );
            m.variable_count = 2;
            m.objective_count = 2;
            m.constraint_count = 0;
            m.variables[0].minimum = 0;
            m.variables[0].maximum = 8;
            m.variables[0].quantum = 1;
            m.variables[1] = m.variables[0];
            m.variables[1].id = 999;
            m = m.seal().unwrap();
            let a = run_search(&m, &synthetic(m), &[]).unwrap();
            let b = run_search_with_workers(&m, &synthetic(m), &[], 4).unwrap();
            let c = run_search_with_workers(&m, &synthetic(m), &[], 8).unwrap();
            assert_eq!(a, b);
            assert_eq!(a, c)
        }
    }
}
