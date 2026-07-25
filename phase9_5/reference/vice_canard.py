#!/usr/bin/env python3
"""Run the finite Phase 9.5 canard exactness probe under VICE."""
from __future__ import annotations
import argparse,json,struct,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import run_prg_until_result
MAGIC=0x394e4143
LAST={"magic":None,"failures":None,"stage":None}

def parse(memory:bytes):
 magic,failures,stage=struct.unpack_from('<III',memory)
 LAST.update(magic=f'0x{magic:08x}',failures=failures,stage=stage)
 if magic!=MAGIC:return None
 if failures:raise RuntimeError(f'Phase 9.5 canard probe reported {failures} failures')
 return {'failures':failures,'stage':stage}
def main():
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--timeout',type=float,default=90.0);a=p.parse_args()
 try:
  result=run_prg_until_result(a.vice.resolve(strict=True),a.prg.resolve(strict=True),a.timeout,0xc000,0xc00b,parse)
 except TimeoutError as error:
  raise TimeoutError(f'{error}; last memory {LAST}') from error
 print(json.dumps(result,indent=2));return 0
if __name__=='__main__':raise SystemExit(main())
