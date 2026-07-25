#!/usr/bin/env python3
"""Validate the stock-C64 Phase 8 seven-page replay shell."""
from __future__ import annotations
import argparse,json,struct,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import run_prg_until_result
MAGIC=0x38554B53;START=0xC800;END=START+19

def parse(memory:bytes):
 if struct.unpack_from('<I',memory,0)[0]!=MAGIC:return None
 schema,status,crc,points,pages,reserved=struct.unpack_from('<HHIIHH',memory,4)
 if(schema,status,reserved)!=(1,0,0):raise RuntimeError(f'invalid replay result {schema=} {status=} {reserved=}')
 return {'screen_crc32':f'{crc:08x}','point_count':points,'page_count':pages}
def main():
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--output',type=Path);p.add_argument('--check',action='store_true');a=p.parse_args();r=run_prg_until_result(a.vice.resolve(strict=True),a.prg.resolve(strict=True),90.0,START,END,parse);data={'schema':'ksa64.phase8.stock-replay-v1',**r};text=json.dumps(data,indent=2)+'\n';print(text,end='')
 if a.check:
  if not a.output:raise RuntimeError('--check requires --output')
  if json.loads(a.output.read_text())!=data:raise RuntimeError(f'evidence differs from {a.output}')
 elif a.output:a.output.write_text(text)
 return 0 if r['page_count']==7 and r['point_count']>0 else 2
if __name__=='__main__':raise SystemExit(main())
