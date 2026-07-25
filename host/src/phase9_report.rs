//! Passive Phase 9 JSON, CSV, and self-contained HTML reports.
use crate::phase9_search::SearchResult;
use ksa64_core::phase9_contract::SearchManifest;
use serde_json::{json, Value};

pub fn report_json(manifest: &SearchManifest, result: &SearchResult) -> Value {
    let last = result.generations.last();
    json!({"schema":"KSA64 Phase 9 report v1","manifest_identity":format!("{:08x}",manifest.identity),"engine":manifest.engine as u8,"seed":manifest.master_seed,"generations":result.generations.len(),"evaluations":result.evaluations,"cache_hits":result.cache_hits,"pareto":result.pareto_indices,"terminal_candidates":last.map(|g|g.candidates.len()).unwrap_or(0),"generation_crc32":result.generations.iter().map(|g|format!("{:08x}",g.crc32)).collect::<Vec<_>>(),"finalists":result.finalists.iter().map(|f|json!({"candidate":format!("{:08x}",f.aggregate.candidate_identity),"feasible":f.aggregate.feasible,"tier":f.aggregate.uncertainty_tier,"objectives":f.aggregate.objectives[..f.aggregate.objective_count as usize].to_vec(),"constraints":f.aggregate.constraint_values[..f.aggregate.constraint_count as usize].to_vec()})).collect::<Vec<_>>()})
}
pub fn report_csv(result: &SearchResult) -> String {
    let mut out=String::from("generation,index,candidate,feasible,fatal,violations,tier,objective0,objective1,objective2,objective3\n");
    for g in &result.generations {
        for (i, a) in g.aggregates.iter().enumerate() {
            out.push_str(&format!(
                "{},{},{:08x},{},{},{},{},{},{},{},{}\n",
                g.index,
                i,
                a.candidate_identity,
                a.feasible,
                a.fatal_class,
                a.violated_constraints,
                a.uncertainty_tier,
                a.objectives[0],
                a.objectives[1],
                a.objectives[2],
                a.objectives[3]
            ))
        }
    }
    out
}
pub fn report_html(manifest: &SearchManifest, result: &SearchResult) -> String {
    let data = serde_json::to_string(&report_json(manifest, result)).unwrap();
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>KSA64 Phase 9</title><style>body{{background:#07111f;color:#d8f3ff;font:15px system-ui;margin:0}}header{{padding:24px;background:linear-gradient(120deg,#102b4e,#32174d)}}main{{padding:24px;display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:16px}}section{{background:#0c1b2d;border:1px solid #24506f;border-radius:12px;padding:16px}}canvas{{width:100%;height:300px;background:#06101b}}code{{color:#7fffd4}}.ok{{color:#7cff8a}}table{{border-collapse:collapse;width:100%}}td,th{{padding:6px;border-bottom:1px solid #234}}</style></head><body><header><h1>🚀 KSA64 Phase 9 Optimization Workbench</h1><div>Manifest <code>{:08x}</code> · deterministic evidence report</div></header><main><section><h2>Experiment</h2><div id="summary"></div></section><section><h2>Pareto projection</h2><canvas id="plot" width="700" height="300"></canvas></section><section><h2>Generation integrity</h2><div id="generations"></div></section><section><h2>64-case finalists</h2><div id="finalists"></div></section></main><script>var d={data};document.getElementById('summary').innerHTML='<p>Engine '+d.engine+' · seed '+d.seed+'</p><p>'+d.evaluations+' evaluations, '+d.cache_hits+' cache hits</p><p class="ok">'+d.finalists.filter(function(x){{return x.feasible}}).length+' feasible finalists</p>';document.getElementById('generations').innerHTML='<ol>'+d.generation_crc32.map(function(x,i){{return '<li>Generation '+i+' — <code>'+x+'</code></li>'}}).join('')+'</ol>';document.getElementById('finalists').innerHTML='<table><tr><th>Candidate</th><th>Tier</th><th>Objectives</th></tr>'+d.finalists.map(function(x){{return '<tr><td><code>'+x.candidate+'</code></td><td>'+x.tier+'</td><td>'+x.objectives.join(' · ')+'</td></tr>'}}).join('')+'</table>';var c=document.getElementById('plot'),ctx=c.getContext('2d');ctx.strokeStyle='#24506f';for(var x=0;x<c.width;x+=70){{ctx.beginPath();ctx.moveTo(x,0);ctx.lineTo(x,c.height);ctx.stroke()}}for(var y=0;y<c.height;y+=50){{ctx.beginPath();ctx.moveTo(0,y);ctx.lineTo(c.width,y);ctx.stroke()}}ctx.fillStyle='#ff77d8';d.finalists.forEach(function(x){{var a=x.objectives[0]||0,b=x.objectives[1]||0,px=30+(Math.abs(a)%640),py=270-(Math.abs(b)%240);ctx.beginPath();ctx.arc(px,py,5,0,Math.PI*2);ctx.fill()}});</script></body></html>"#,
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
        let m = built_in_manifest(
            StudyId::PassiveRecovery,
            SearchEngineId::GridV1,
            SearchPresetId::Quick,
        );
        let r = SearchResult {
            manifest_identity: m.identity,
            generations: vec![],
            pareto_indices: vec![],
            finalists: vec![],
            cache_hits: 0,
            evaluations: 0,
        };
        let h = report_html(&m, &r);
        assert!(h.contains("<!doctype html>"));
        assert!(!h.contains("src=\"http"));
        assert!(report_csv(&r).starts_with("generation"));
        assert_eq!(
            report_json(&m, &r)["manifest_identity"],
            format!("{:08x}", m.identity)
        )
    }
}
