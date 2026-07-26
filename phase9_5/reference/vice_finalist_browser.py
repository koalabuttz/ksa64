#!/usr/bin/env python3
"""Finite stock-C64 Phase 9.5 advanced-finalist browser acceptance probe."""
from __future__ import annotations
import argparse,hashlib,json,struct,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import run_prg_until_result
MAGIC=0x39464539;START=0xC800;END=START+19
def parse(memory:bytes):
 if struct.unpack_from('<I',memory,0)[0]!=MAGIC:return None
 status,code,count=struct.unpack_from('<HHH',memory,4);manifest=struct.unpack_from('<I',memory,12)[0];study=struct.unpack_from('<I',memory,16)[0]
 if status!=0:raise RuntimeError(f'advanced finalist browser failed: {code}')
 return {'status':status,'code':code,'finalist_count':count,'manifest_identity':f'{manifest:08x}','study_identity':f'{study:08x}'}
def main():
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--output',type=Path);p.add_argument('--check',action='store_true');a=p.parse_args();prg=a.prg.resolve(strict=True);r=run_prg_until_result(a.vice.resolve(strict=True),prg,30.0,START,END,parse);raw=prg.read_bytes();load=struct.unpack_from('<H',raw,0)[0];data={'schema':'ksa64.phase9_5.finalist-browser-v1',**r,'artifact':{'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'load_address':load,'load_end_exclusive':load+len(raw)-2,'stock_fit':load+len(raw)-2<=0xC000},'reu_required':False};text=json.dumps(data,indent=2)+'\n';print(text,end='')
 if a.check:
  if not a.output:raise RuntimeError('--check requires --output')
  if json.loads(a.output.read_text())!=data:raise RuntimeError(f'evidence differs from {a.output}')
 elif a.output:a.output.write_text(text)
 return 0
if __name__=='__main__':raise SystemExit(main())
