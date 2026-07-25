#!/usr/bin/env python3
"""Run the finite Phase 9.5 advanced-avionics probe under VICE."""
from __future__ import annotations
import argparse,json,struct,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import run_prg_until_result
MAGIC=0x39464441
LAST={"magic":None,"failures":None,"signature":None}
def parse(memory:bytes):
 magic,failures,signature=struct.unpack_from('<III',memory);LAST.update(magic=f'0x{magic:08x}',failures=failures,signature=f'0x{signature:08x}')
 if magic!=MAGIC:return None
 if failures:raise RuntimeError(f'advanced-flight mismatch mask 0x{failures:08x}')
 return {'failures':failures,'signature':f'0x{signature:08x}'}
def main():
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--timeout',type=float,default=90.0);a=p.parse_args()
 try:r=run_prg_until_result(a.vice.resolve(strict=True),a.prg.resolve(strict=True),a.timeout,0xc000,0xc00b,parse)
 except TimeoutError as e:raise TimeoutError(f'{e}; last memory {LAST}') from e
 print(json.dumps(r,indent=2));return 0
if __name__=='__main__':raise SystemExit(main())
