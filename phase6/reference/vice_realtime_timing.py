#!/usr/bin/env python3
from __future__ import annotations
import argparse, hashlib, json, struct, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import run_prg_until_result
MAGIC=0x3652544B; START=0xC000; END=0xC023; PAL=985_248; FAST_TICKS=12_692

def parse(memory:bytes):
    if struct.unpack_from('<I',memory,0)[0]!=MAGIC:return None
    schema,status=struct.unpack_from('<HH',memory,4)
    if schema!=1 or status:raise RuntimeError(f'invalid realtime timing result schema={schema} status={status}')
    overhead,navigation,fast,guidance,budget,nav,flight=struct.unpack_from('<IIIIIII',memory,8)
    return {'overhead':overhead,'navigation_cycles':navigation,'fast_cycles':fast,'guidance_cycles':guidance,'budget_cycles':budget,'navigation_checksum':nav,'flight_checksum':flight}

def main():
    p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--runs',type=int,default=3);p.add_argument('--output',type=Path);p.add_argument('--check',action='store_true');a=p.parse_args()
    samples=[run_prg_until_result(a.vice.resolve(strict=True),a.prg.resolve(strict=True),120,START,END,parse) for _ in range(a.runs)]
    if len({json.dumps(s,sort_keys=True) for s in samples})!=1:raise RuntimeError(f'nondeterministic timing: {samples}')
    r=samples[0]; nav_ticks=(FAST_TICKS+3)//4; guidance_ticks=(FAST_TICKS+31)//32; fast_only=FAST_TICKS-nav_ticks-guidance_ticks; projected=(nav_ticks*r['navigation_cycles']+guidance_ticks*r['guidance_cycles']+fast_only*r['fast_cycles'])/PAL
    data={'schema':'ksa64.phase6.realtime-timing-v1','target':'PAL C64 via pinned x64sc 3.10','runs':a.runs,'fast_ticks':FAST_TICKS,'cycles':r,'deadline_pass':max(r['navigation_cycles'],r['guidance_cycles'],r['fast_cycles'])<=r['budget_cycles'],'projected_cpu_seconds':projected,'physical_mission_seconds':FAST_TICKS/32,'artifact':{'bytes':a.prg.stat().st_size,'sha256':hashlib.sha256(a.prg.read_bytes()).hexdigest()}}
    text=json.dumps(data,indent=2)+'\n';print(text,end='')
    if a.check:
        if not a.output:raise RuntimeError('--check requires --output')
        if json.loads(a.output.read_text())!=data:raise RuntimeError(f'timing evidence differs from {a.output}')
    elif a.output:a.output.write_text(text)
    if not data['deadline_pass']:return 2
    return 0
if __name__=='__main__':raise SystemExit(main())
