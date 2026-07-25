//! Passive Phase 9 JSON, CSV, and self-contained HTML reports.
use crate::phase9_search::SearchResult;
use crate::phase9_sensitivity::SensitivityRecord;
use ksa64_core::phase9_contract::SearchManifest;
use serde_json::{json, Value};

pub fn report_json(manifest: &SearchManifest, result: &SearchResult) -> Value {
    report_json_with_sensitivity(manifest, result, &[])
}

pub fn report_json_with_sensitivity(
    manifest: &SearchManifest,
    result: &SearchResult,
    sensitivity: &[SensitivityRecord],
) -> Value {
    let last = result.generations.last();
    json!({
        "schema":"KSA64 Phase 9 report v2",
        "manifest_identity":format!("{:08x}",manifest.identity),
        "engine":manifest.engine as u8,
        "preset":manifest.preset as u8,
        "seed":manifest.master_seed,
        "budgets":{
            "grid_points":manifest.budgets.grid_points,
            "population":manifest.budgets.population,
            "generations":manifest.budgets.generations,
            "finalists":manifest.budgets.finalists,
            "max_candidates":manifest.budgets.max_candidates,
        },
        "variables":manifest.variables[..manifest.variable_count as usize].iter().map(|value|json!({
            "id":value.id,"kind":value.kind as u8,"minimum":value.minimum,"maximum":value.maximum,
            "quantum":value.quantum,"catalogue_identity":format!("{:08x}",value.catalogue_id)
        })).collect::<Vec<_>>(),
        "objective_contract":manifest.objectives[..manifest.objective_count as usize].iter().map(|value|json!({
            "metric":value.metric_id,"aggregate":value.aggregate as u8,"direction":value.direction as u8
        })).collect::<Vec<_>>(),
        "constraint_contract":manifest.constraints[..manifest.constraint_count as usize].iter().map(|value|json!({
            "metric":value.metric_id,"aggregate":value.aggregate as u8,"operator":value.op as u8,
            "threshold":value.threshold,"scale":value.scale
        })).collect::<Vec<_>>(),
        "generations":result.generations.len(),
        "evaluations":result.evaluations,
        "cache_hits":result.cache_hits,
        "pareto":result.pareto_indices,
        "terminal_candidates":last.map(|generation|generation.aggregates.iter().map(|value|json!({
            "candidate":format!("{:08x}",value.candidate_identity),"feasible":value.feasible,
            "fatal":value.fatal_class,"violations":value.violated_constraints,
            "objectives":value.objectives[..value.objective_count as usize].to_vec(),
            "constraints":value.constraint_values[..value.constraint_count as usize].to_vec(),
        })).collect::<Vec<_>>()).unwrap_or_default(),
        "generation_evidence":result.generations.iter().map(|generation|json!({
            "index":generation.index,"candidates":generation.candidates.len(),
            "feasible":generation.aggregates.iter().filter(|value|value.feasible).count(),
            "crc32":format!("{:08x}",generation.crc32),
        })).collect::<Vec<_>>(),
        "generation_crc32":result.generations.iter().map(|generation|format!("{:08x}",generation.crc32)).collect::<Vec<_>>(),
        "retained_case_evidence":result.evidence.iter().map(|value|json!({
            "candidate":format!("{:08x}",value.aggregate.candidate_identity),
            "tier":value.aggregate.uncertainty_tier,"records":value.cases.len()
        })).collect::<Vec<_>>(),
        "finalists":result.finalists.iter().map(|value|json!({
            "candidate":format!("{:08x}",value.aggregate.candidate_identity),
            "feasible":value.aggregate.feasible,"tier":value.aggregate.uncertainty_tier,
            "objectives":value.aggregate.objectives[..value.aggregate.objective_count as usize].to_vec(),
            "constraints":value.aggregate.constraint_values[..value.aggregate.constraint_count as usize].to_vec()
        })).collect::<Vec<_>>(),
        "sensitivity":sensitivity.iter().map(|value|json!({
            "candidate":format!("{:08x}",value.candidate_identity),"variable":value.variable_id,
            "objective":value.objective_index,"flags":value.flags,"baseline":value.baseline,
            "lower":value.lower,"upper":value.upper,"delta_lower":value.delta_lower,
            "delta_upper":value.delta_upper,"slope_q16":value.slope_q16
        })).collect::<Vec<_>>(),
    })
}

pub fn report_csv(result: &SearchResult) -> String {
    let mut output=String::from("generation,index,candidate,feasible,fatal,violations,tier,objective0,objective1,objective2,objective3\n");
    for generation in &result.generations {
        for (index, aggregate) in generation.aggregates.iter().enumerate() {
            output.push_str(&format!(
                "{},{},{:08x},{},{},{},{},{},{},{},{}\n",
                generation.index,
                index,
                aggregate.candidate_identity,
                aggregate.feasible,
                aggregate.fatal_class,
                aggregate.violated_constraints,
                aggregate.uncertainty_tier,
                aggregate.objectives[0],
                aggregate.objectives[1],
                aggregate.objectives[2],
                aggregate.objectives[3]
            ))
        }
    }
    output
}

pub fn report_html(manifest: &SearchManifest, result: &SearchResult) -> String {
    report_html_with_sensitivity(manifest, result, &[])
}

pub fn report_html_with_sensitivity(
    manifest: &SearchManifest,
    result: &SearchResult,
    sensitivity: &[SensitivityRecord],
) -> String {
    let data = serde_json::to_string(&report_json_with_sensitivity(manifest, result, sensitivity))
        .unwrap();
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>KSA64 Phase 9</title><style>
:root{{--bg:#050b14;--panel:#0b1726;--line:#1f4a68;--text:#d9f5ff;--muted:#7fa4b8;--pink:#ff73d1;--cyan:#5ee7ff;--green:#77f59a}}*{{box-sizing:border-box}}body{{margin:0;background:radial-gradient(circle at 20% 0,#102a46,var(--bg) 42%);color:var(--text);font:14px ui-monospace,Consolas,monospace}}header{{padding:30px clamp(18px,5vw,64px);border-bottom:1px solid var(--line);background:linear-gradient(120deg,#0d2843cc,#321448cc)}}h1{{margin:0 0 8px;font-size:clamp(25px,4vw,48px)}}main{{padding:24px clamp(14px,4vw,52px);display:grid;grid-template-columns:repeat(12,1fr);gap:16px}}section{{grid-column:span 4;background:#0b1726e8;border:1px solid var(--line);border-radius:14px;padding:16px;box-shadow:0 12px 28px #0006}}.wide{{grid-column:span 8}}.full{{grid-column:1/-1}}@media(max-width:1000px){{section,.wide{{grid-column:1/-1}}}}canvas{{width:100%;height:320px;background:#06101b;border-radius:8px}}code{{color:var(--cyan)}}.ok{{color:var(--green)}}.muted{{color:var(--muted)}}table{{border-collapse:collapse;width:100%;font-size:12px}}td,th{{padding:6px;border-bottom:1px solid #18344a;text-align:right}}td:first-child,th:first-child{{text-align:left}}select{{background:#07111e;color:var(--text);border:1px solid var(--line);padding:5px}}pre{{white-space:pre-wrap;max-height:360px;overflow:auto;color:#a9cbd9}}.cards{{display:grid;grid-template-columns:repeat(2,1fr);gap:10px}}.card{{padding:12px;background:#07111e;border-radius:8px}}.big{{font-size:24px;color:var(--cyan)}}
</style></head><body><header><h1>🚀 KSA64 Optimization Workbench</h1><div>Manifest <code>{:08x}</code> · deterministic, feasibility-first evidence</div></header><main>
<section><h2>Experiment</h2><div id="summary" class="cards"></div></section>
<section class="wide"><h2>Pareto explorer</h2><label>X <select id="xaxis"></select></label> <label>Y <select id="yaxis"></select></label><canvas id="pareto" width="900" height="320"></canvas></section>
<section class="wide"><h2>Convergence and feasibility</h2><canvas id="convergence" width="900" height="320"></canvas></section>
<section><h2>Integrity</h2><div id="integrity"></div></section>
<section class="wide"><h2>64-case finalists</h2><div id="finalists"></div></section>
<section><h2>Local sensitivity</h2><div id="sensitivity"></div></section>
<section class="full"><h2>Compiled manifest</h2><pre id="manifest"></pre></section>
</main><script>
var d={data};
function card(label,value){{return '<div class="card"><div class="muted">'+label+'</div><div class="big">'+value+'</div></div>'}}
document.getElementById('summary').innerHTML=card('Evaluations',d.evaluations)+card('Cache hits',d.cache_hits)+card('Generations',d.generations)+card('64-case finalists',d.finalists.length);
document.getElementById('integrity').innerHTML='<p class="ok">'+d.retained_case_evidence.length+' candidate evidence streams retained</p><ol>'+d.generation_evidence.map(function(g){{return '<li>G'+g.index+' · '+g.candidates+' candidates · '+g.feasible+' feasible · <code>'+g.crc32+'</code></li>'}}).join('')+'</ol>';
document.getElementById('finalists').innerHTML='<table><tr><th>Candidate</th><th>Tier</th><th>Objectives</th><th>Constraints</th></tr>'+d.finalists.map(function(x){{return '<tr><td><code>'+x.candidate+'</code></td><td>'+x.tier+'</td><td>'+x.objectives.join(' · ')+'</td><td>'+x.constraints.join(' · ')+'</td></tr>'}}).join('')+'</table>';
document.getElementById('sensitivity').innerHTML='<table><tr><th>Variable</th><th>Objective</th><th>Slope Q16</th></tr>'+d.sensitivity.slice(0,32).map(function(x){{return '<tr><td>'+x.variable+'</td><td>'+x.objective+'</td><td>'+x.slope_q16+'</td></tr>'}}).join('')+'</table>';
document.getElementById('manifest').textContent=JSON.stringify({{seed:d.seed,budgets:d.budgets,variables:d.variables,objectives:d.objective_contract,constraints:d.constraint_contract}},null,2);
var objectiveCount=d.objective_contract.length,xa=document.getElementById('xaxis'),ya=document.getElementById('yaxis');for(var i=0;i<objectiveCount;i++){{xa.add(new Option('Objective '+i,i));ya.add(new Option('Objective '+i,i))}}ya.value=String(Math.min(1,objectiveCount-1));
function axes(ctx,w,h){{ctx.clearRect(0,0,w,h);ctx.strokeStyle='#173850';ctx.lineWidth=1;for(var i=0;i<=10;i++){{ctx.beginPath();ctx.moveTo(36+i*(w-52)/10,10);ctx.lineTo(36+i*(w-52)/10,h-28);ctx.stroke();ctx.beginPath();ctx.moveTo(36,10+i*(h-38)/10);ctx.lineTo(w-16,10+i*(h-38)/10);ctx.stroke()}}}}
function scatter(){{var c=document.getElementById('pareto'),ctx=c.getContext('2d'),x=+xa.value,y=+ya.value,pts=d.terminal_candidates.filter(function(v){{return v.feasible}});axes(ctx,c.width,c.height);if(!pts.length)return;var xs=pts.map(v=>v.objectives[x]),ys=pts.map(v=>v.objectives[y]),xmin=Math.min(...xs),xmax=Math.max(...xs),ymin=Math.min(...ys),ymax=Math.max(...ys);pts.forEach(function(v){{var px=36+(v.objectives[x]-xmin)/(xmax-xmin||1)*(c.width-52),py=c.height-28-(v.objectives[y]-ymin)/(ymax-ymin||1)*(c.height-38);ctx.fillStyle=d.finalists.some(f=>f.candidate===v.candidate)?'#ff73d1':'#5ee7ff';ctx.beginPath();ctx.arc(px,py,5,0,Math.PI*2);ctx.fill()}})}}xa.onchange=scatter;ya.onchange=scatter;scatter();
(function(){{var c=document.getElementById('convergence'),ctx=c.getContext('2d');axes(ctx,c.width,c.height);var values=d.generation_evidence.map(function(g){{return g.feasible}});var min=Math.min(...values),max=Math.max(...values);ctx.strokeStyle='#77f59a';ctx.lineWidth=3;ctx.beginPath();values.forEach(function(v,i){{var x=36+i/(values.length-1||1)*(c.width-52),y=c.height-28-(v-min)/(max-min||1)*(c.height-38);i?ctx.lineTo(x,y):ctx.moveTo(x,y)}});ctx.stroke()}})();
</script></body></html>"#,
        manifest.identity
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase9::{built_in_manifest, StudyId};
    use crate::phase9_search::SearchResult;
    use ksa64_core::phase9_contract::{SearchEngineId, SearchPresetId};
    #[test]
    fn reports_are_self_contained() {
        let manifest = built_in_manifest(
            StudyId::PassiveRecovery,
            SearchEngineId::GridV1,
            SearchPresetId::Quick,
        );
        let result = SearchResult {
            manifest_identity: manifest.identity,
            generations: vec![],
            pareto_indices: vec![],
            finalists: vec![],
            evidence: vec![],
            cache_hits: 0,
            evaluations: 0,
        };
        let html = report_html(&manifest, &result);
        assert!(html.contains("<!doctype html>"));
        assert!(!html.contains("src=\"http"));
        assert!(report_csv(&result).starts_with("generation"));
        assert_eq!(
            report_json(&manifest, &result)["manifest_identity"],
            format!("{:08x}", manifest.identity)
        );
    }
}
