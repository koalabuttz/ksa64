#!/usr/bin/env python3
"""Run the finite Phase 9.5 numeric/wire-contract probe under VICE."""
from __future__ import annotations
import argparse,json,struct,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import run_prg_until_result
MAGIC=0x39434c4b
EXPECTED=0xde42a746

def parse(memory:bytes):
 magic,failures,signature=struct.unpack_from('<III',memory)
 if magic!=MAGIC:return None
 if failures:raise RuntimeError(f'Phase 9.5 contract probe reported {failures} failures')
 if signature!=EXPECTED:raise RuntimeError(f'signature 0x{signature:08x}, expected 0x{EXPECTED:08x}')
 return {'failures':failures,'signature':f'0x{signature:08x}'}
def main():
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--timeout',type=float,default=90.0);a=p.parse_args()
 result=run_prg_until_result(a.vice.resolve(strict=True),a.prg.resolve(strict=True),a.timeout,0xc000,0xc00b,parse)
 print(json.dumps(result,indent=2));return 0
if __name__=='__main__':raise SystemExit(main())
