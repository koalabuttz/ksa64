//! Persistent bounded JSONL evaluator service for external optimizers.
use crate::phase9_search::{CandidateEvaluator, SearchError};
use ksa64_core::phase9_contract::{DesignVector, SearchManifest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Write};

pub const MAX_JSONL_LINE: usize = 64 * 1024;
#[derive(Debug)]
pub enum ProtocolError {
    Io,
    LineTooLong,
    Output,
}
#[derive(Deserialize)]
struct Request {
    kind: String,
    request_id: Option<u64>,
    tier: Option<u8>,
    values: Option<BTreeMap<u16, i32>>,
}
#[derive(Serialize)]
struct Response<'a> {
    kind: &'a str,
    request_id: Option<u64>,
    ok: bool,
    message: &'a str,
    manifest_identity: u32,
    candidate_identity: Option<u32>,
    feasible: Option<bool>,
    objectives: Option<Vec<i32>>,
    constraints: Option<Vec<i32>>,
}

pub fn serve_jsonl<R: BufRead, W: Write, E: CandidateEvaluator>(
    manifest: &SearchManifest,
    evaluator: &E,
    mut input: R,
    mut output: W,
    transcript: &mut Vec<u8>,
) -> Result<(), ProtocolError> {
    let mut line = Vec::new();
    let mut ids = BTreeSet::new();
    loop {
        line.clear();
        let n = input
            .read_until(b'\n', &mut line)
            .map_err(|_| ProtocolError::Io)?;
        if n == 0 {
            break;
        }
        if line.len() > MAX_JSONL_LINE {
            respond(
                &mut output,
                transcript,
                Response {
                    kind: "error",
                    request_id: None,
                    ok: false,
                    message: "line_too_long",
                    manifest_identity: manifest.identity,
                    candidate_identity: None,
                    feasible: None,
                    objectives: None,
                    constraints: None,
                },
            )?;
            continue;
        }
        transcript.extend_from_slice(&line);
        let request: Request = match serde_json::from_slice(&line) {
            Ok(v) => v,
            Err(_) => {
                respond(
                    &mut output,
                    transcript,
                    Response {
                        kind: "error",
                        request_id: None,
                        ok: false,
                        message: "malformed_json",
                        manifest_identity: manifest.identity,
                        candidate_identity: None,
                        feasible: None,
                        objectives: None,
                        constraints: None,
                    },
                )?;
                continue;
            }
        };
        let id = request.request_id;
        if let Some(v) = id {
            if !ids.insert(v) {
                respond(
                    &mut output,
                    transcript,
                    Response {
                        kind: "error",
                        request_id: id,
                        ok: false,
                        message: "duplicate_request_id",
                        manifest_identity: manifest.identity,
                        candidate_identity: None,
                        feasible: None,
                        objectives: None,
                        constraints: None,
                    },
                )?;
                continue;
            }
        }
        match request.kind.as_str() {
            "hello" => respond(
                &mut output,
                transcript,
                Response {
                    kind: "hello",
                    request_id: id,
                    ok: true,
                    message: "ksa64-phase9-jsonl-v1",
                    manifest_identity: manifest.identity,
                    candidate_identity: None,
                    feasible: None,
                    objectives: None,
                    constraints: None,
                },
            )?,
            "evaluate" => {
                let supplied = match request.values {
                    Some(v) => v,
                    None => {
                        respond(
                            &mut output,
                            transcript,
                            Response {
                                kind: "error",
                                request_id: id,
                                ok: false,
                                message: "missing_values",
                                manifest_identity: manifest.identity,
                                candidate_identity: None,
                                feasible: None,
                                objectives: None,
                                constraints: None,
                            },
                        )?;
                        continue;
                    }
                };
                let mut raw = [0; 32];
                let mut valid = supplied.len() == manifest.variable_count as usize;
                for i in 0..manifest.variable_count as usize {
                    let spec = manifest.variables[i];
                    match supplied.get(&spec.id) {
                        Some(v) if spec.accepts(*v) => raw[i] = *v,
                        _ => valid = false,
                    }
                }
                if !valid {
                    respond(
                        &mut output,
                        transcript,
                        Response {
                            kind: "error",
                            request_id: id,
                            ok: false,
                            message: "invalid_candidate",
                            manifest_identity: manifest.identity,
                            candidate_identity: None,
                            feasible: None,
                            objectives: None,
                            constraints: None,
                        },
                    )?;
                    continue;
                }
                let candidate = DesignVector {
                    identity: 0,
                    manifest_identity: manifest.identity,
                    value_count: manifest.variable_count,
                    values: raw,
                    materialized_ids: [0; 4],
                }
                .seal()
                .map_err(|_| ProtocolError::Output)?;
                match evaluator.evaluate(&candidate, request.tier.unwrap_or(8)) {
                    Ok(v) => respond(
                        &mut output,
                        transcript,
                        Response {
                            kind: "evaluation",
                            request_id: id,
                            ok: true,
                            message: "ok",
                            manifest_identity: manifest.identity,
                            candidate_identity: Some(candidate.identity),
                            feasible: Some(v.aggregate.feasible),
                            objectives: Some(
                                v.aggregate.objectives[..v.aggregate.objective_count as usize]
                                    .to_vec(),
                            ),
                            constraints: Some(
                                v.aggregate.constraint_values
                                    [..v.aggregate.constraint_count as usize]
                                    .to_vec(),
                            ),
                        },
                    )?,
                    Err(SearchError::Evaluation) => respond(
                        &mut output,
                        transcript,
                        Response {
                            kind: "error",
                            request_id: id,
                            ok: false,
                            message: "evaluation_failed",
                            manifest_identity: manifest.identity,
                            candidate_identity: Some(candidate.identity),
                            feasible: None,
                            objectives: None,
                            constraints: None,
                        },
                    )?,
                    Err(_) => respond(
                        &mut output,
                        transcript,
                        Response {
                            kind: "error",
                            request_id: id,
                            ok: false,
                            message: "configuration_error",
                            manifest_identity: manifest.identity,
                            candidate_identity: Some(candidate.identity),
                            feasible: None,
                            objectives: None,
                            constraints: None,
                        },
                    )?,
                }
            }
            "checkpoint" => respond(
                &mut output,
                transcript,
                Response {
                    kind: "checkpoint",
                    request_id: id,
                    ok: true,
                    message: "completed_boundary",
                    manifest_identity: manifest.identity,
                    candidate_identity: None,
                    feasible: None,
                    objectives: None,
                    constraints: None,
                },
            )?,
            "close" => {
                respond(
                    &mut output,
                    transcript,
                    Response {
                        kind: "close",
                        request_id: id,
                        ok: true,
                        message: "closed",
                        manifest_identity: manifest.identity,
                        candidate_identity: None,
                        feasible: None,
                        objectives: None,
                        constraints: None,
                    },
                )?;
                break;
            }
            _ => respond(
                &mut output,
                transcript,
                Response {
                    kind: "error",
                    request_id: id,
                    ok: false,
                    message: "unknown_message",
                    manifest_identity: manifest.identity,
                    candidate_identity: None,
                    feasible: None,
                    objectives: None,
                    constraints: None,
                },
            )?,
        }
    }
    Ok(())
}
fn respond<W: Write>(
    w: &mut W,
    transcript: &mut Vec<u8>,
    response: Response<'_>,
) -> Result<(), ProtocolError> {
    let mut bytes = serde_json::to_vec(&response).map_err(|_| ProtocolError::Output)?;
    bytes.push(b'\n');
    w.write_all(&bytes).map_err(|_| ProtocolError::Io)?;
    w.flush().map_err(|_| ProtocolError::Io)?;
    transcript.extend_from_slice(&bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase9::{baseline_vector, built_in_manifest, CandidateEvaluation, StudyId};
    use ksa64_core::phase9_contract::{CandidateAggregate, SearchEngineId, SearchPresetId};
    #[test]
    fn malformed_input_does_not_end_session() {
        let m = built_in_manifest(
            StudyId::GimbalControl,
            SearchEngineId::GridV1,
            SearchPresetId::Quick,
        );
        let b = baseline_vector(&m);
        let mut map = serde_json::Map::new();
        for i in 0..m.variable_count as usize {
            map.insert(
                m.variables[i].id.to_string(),
                serde_json::json!(b.values[i]),
            );
        }
        let lines = format!(
            "not-json\n{}\n{}\n{}\n",
            serde_json::json!({"kind":"hello","request_id":1}),
            serde_json::json!({"kind":"evaluate","request_id":2,"tier":1,"values":map}),
            serde_json::json!({"kind":"close","request_id":3})
        );
        let eval = |d: &DesignVector, t: u8| {
            let a = CandidateAggregate {
                identity: 0,
                manifest_identity: m.identity,
                candidate_identity: d.identity,
                uncertainty_tier: t,
                case_count: t,
                fatal_class: 0,
                violated_constraints: 0,
                feasible: true,
                case_crc: 0,
                normalized_violation: 0,
                objective_count: m.objective_count,
                constraint_count: m.constraint_count,
                objectives: [0; 8],
                constraint_values: [0; 16],
            }
            .seal();
            Ok(CandidateEvaluation {
                aggregate: a,
                cases: vec![],
            })
        };
        let mut out = Vec::new();
        let mut transcript = Vec::new();
        serve_jsonl(
            &m,
            &eval,
            std::io::Cursor::new(lines),
            &mut out,
            &mut transcript,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("malformed_json"));
        assert!(text.contains("evaluation"));
        assert!(text.contains("closed"));
    }
}
