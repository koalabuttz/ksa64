#!/usr/bin/env python3
from __future__ import annotations
import argparse,hashlib,json,struct,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import run_prg_until_result
MAGIC=0x38544C4B;START=0xC000;END=0xC01F;PAL=985_248

def parse(memory:bytes):
 if struct.unpack_from('<I',memory,0)[0]!=MAGIC:return None
 schema,status=struct.unpack_from('<HH',memory,4)
 if schema!=1:raise RuntimeError(f'Phase 8.5 timing schema={schema}')
 overhead,aided,fast,budget,nav,flight=struct.unpack_from('<IIIIII',memory,8)
 return {'status':status,'overhead':overhead,'aided_cycles':aided,'fast_cycles':fast,'budget_cycles':budget,'navigation_checksum':nav,'flight_checksum':flight}
def main()->int:
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--runs',type=int,default=3);p.add_argument('--output',type=Path);p.add_argument('--check',action='store_true');a=p.parse_args();vice=a.vice.resolve(strict=True);prg=a.prg.resolve(strict=True)
 samples=[run_prg_until_result(vice,prg,120,START,END,parse) for _ in range(a.runs)]
 if len({json.dumps(v,sort_keys=True) for v in samples})!=1:raise RuntimeError(f'nondeterministic timing: {samples}')
 cycles=samples[0];worst=max(cycles['aided_cycles'],cycles['fast_cycles']);data={'schema':'ksa64.phase8_5.avionics-timing-v1','target':'PAL stock C64 via pinned x64sc 3.10','runs':a.runs,'cycles':cycles,'deadline_pass':worst<=cycles['budget_cycles'],'release_budget_fraction':worst/(PAL/32),'headroom_cycles':cycles['budget_cycles']-worst,'artifact':{'bytes':prg.stat().st_size,'sha256':hashlib.sha256(prg.read_bytes()).hexdigest(),'load_address':struct.unpack_from('<H',prg.read_bytes(),0)[0]}}
 text=json.dumps(data,indent=2)+'\n';print(text,end='')
 if a.check:
  if not a.output or json.loads(a.output.read_text())!=data:raise RuntimeError('timing evidence mismatch')
 elif a.output:a.output.write_text(text)
 return 0 if data['deadline_pass'] else 2
if __name__=='__main__':raise SystemExit(main())
