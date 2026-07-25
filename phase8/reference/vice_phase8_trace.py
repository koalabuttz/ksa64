#!/usr/bin/env python3
"""Compare the first 17 Phase 8 states and project a conservative PAL runtime."""
from __future__ import annotations
import argparse,json,struct,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import run_prg_until_result
MAGIC=0x38524B53;START=0xC800;COUNT=17;RECORD_BASE=16;LAST_BASE=RECORD_BASE+COUNT*4;END=START+LAST_BASE+79;PAL_HZ=985248;REFERENCE_STEPS=2244

def parse(memory:bytes):
 if struct.unpack_from('<I',memory,0)[0]!=MAGIC:return None
 schema,status,count,reserved,cycles=struct.unpack_from('<HHHHI',memory,4)
 if (schema,status,count,reserved)!=(1,0,COUNT,0):raise RuntimeError(f'invalid header {schema=} {status=} {count=} {reserved=}')
 return {'checksums':list(struct.unpack_from(f'<{COUNT}I',memory,RECORD_BASE)),'last':list(struct.unpack_from('<20i',memory,LAST_BASE)),'net_cycles':cycles}

def main():
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--host',type=Path,required=True);p.add_argument('--output',type=Path);p.add_argument('--check',action='store_true');a=p.parse_args();h=json.loads(a.host.read_text());t=run_prg_until_result(a.vice.resolve(strict=True),a.prg.resolve(strict=True),300.0,START,END,parse)
 div=next((i for i,(x,y) in enumerate(zip(h['checksums'],t['checksums'],strict=True))if x!=y),None);cycles_per=t['net_cycles']/(COUNT-1);projected=cycles_per*REFERENCE_STEPS/PAL_HZ
 data={'schema':'ksa64.phase8.exact-trace-v1','count':COUNT,'first_divergence':div,'host_last':h['last'],'target_last':t['last'],'exact':div is None and h['last']==t['last'],'net_cycles':t['net_cycles'],'cycles_per_powered_step':round(cycles_per,3),'conservative_reference_steps':REFERENCE_STEPS,'conservative_projected_pal_seconds':round(projected,3),'full_c64_run_permitted_without_confirmation':False,'projection_policy':'A complete run requires projection <= 1800 seconds and explicit user confirmation.'}
 text=json.dumps(data,indent=2)+'\n';print(text,end='')
 if a.check:
  if not a.output:raise RuntimeError('--check requires --output')
  if json.loads(a.output.read_text())!=data:raise RuntimeError(f'evidence differs from {a.output}')
 elif a.output:a.output.write_text(text)
 return 0 if data['exact'] else 2
if __name__=='__main__':raise SystemExit(main())
