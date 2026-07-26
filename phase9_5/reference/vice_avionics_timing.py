#!/usr/bin/env python3
from __future__ import annotations
import argparse,hashlib,json,struct,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import run_prg_until_result
MAGIC=0x39544C4B;START=0xC000;END=0xC023;PAL=985_248

def parse(memory:bytes):
 if struct.unpack_from('<I',memory,0)[0]!=MAGIC:return None
 schema,status=struct.unpack_from('<HH',memory,4)
 if schema!=1:raise RuntimeError(f'Phase 9.5 timing schema={schema}')
 overhead,aided,fast,fallback,worst,budget,allocator=struct.unpack_from('<IIIIIII',memory,8)
 return {'status':status,'overhead':overhead,'aided_cycles':aided,'fast_cycles':fast,'fallback_cycles':fallback,'worst_cycles':worst,'budget_cycles':budget,'allocator_checksum':allocator}
def main()->int:
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--runs',type=int,default=3);p.add_argument('--output',type=Path);a=p.parse_args();vice=a.vice.resolve(strict=True);prg=a.prg.resolve(strict=True)
 samples=[run_prg_until_result(vice,prg,120,START,END,parse) for _ in range(a.runs)]
 if len({json.dumps(v,sort_keys=True) for v in samples})!=1:raise RuntimeError(f'nondeterministic timing: {samples}')
 cycles=samples[0];data={'schema':'ksa64.phase9_5.avionics-timing-v1','target':'PAL stock C64 via pinned x64sc 3.10','runs':a.runs,'cycles':cycles,'deadline_pass':cycles['status']==0 and cycles['worst_cycles']<=cycles['budget_cycles'],'release_budget_fraction':cycles['worst_cycles']/(PAL/32),'headroom_cycles':cycles['budget_cycles']-cycles['worst_cycles'],'artifact':{'bytes':prg.stat().st_size,'sha256':hashlib.sha256(prg.read_bytes()).hexdigest(),'load_address':struct.unpack_from('<H',prg.read_bytes(),0)[0],'load_end_exclusive':struct.unpack_from('<H',prg.read_bytes(),0)[0]+prg.stat().st_size-2}}
 text=json.dumps(data,indent=2)+'\n';print(text,end='')
 if a.output:a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(text)
 return 0 if data['deadline_pass'] else 2
if __name__=='__main__':raise SystemExit(main())
