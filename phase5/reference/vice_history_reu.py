#!/usr/bin/env python3
"""Exercise Phase 5 adaptive history plans across the VICE REU matrix."""
from __future__ import annotations
import argparse,json,pathlib,struct,sys
ROOT=pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'phase4/reference'))
from vice_reu import run_case as run_raw
SIZES=[0,128,256,512,1024,2048,4096,8192,16384]
EXPECTED={0:(5,0,1),128:(204,0,15),256:(256,0,34),512:(256,0,75),1024:(256,0,157),2048:(256,1,113),4096:(256,3,25),8192:(256,6,58),16384:(256,12,123)}
def parse(memory,expected):
 if memory[:4]!=b'H5P0': return None
 version,status=struct.unpack_from('<HH',memory,4); capacity,second=struct.unpack_from('<II',memory,8); preserved=bool(memory[16]); summaries,full,compact=struct.unpack_from('<HHH',memory,18);used,free=struct.unpack_from('<II',memory,24)
 if (version,status,capacity,second,preserved)!=(1,0,expected,expected,True): raise RuntimeError(f'bad Phase 5 REU result {expected}: {(version,status,capacity,second,preserved)}')
 if (summaries,full,compact)!=EXPECTED[expected]: raise RuntimeError(f'bad plan {expected}: {(summaries,full,compact)}')
 if expected and used+free!=expected*1024: raise RuntimeError('capacity accounting mismatch')
 return {'capacity_kib':capacity,'preserved':preserved,'summary_slots':summaries,'full_histories':full,'compact_histories':compact,'used_bytes':used,'free_bytes':free}
def run(vice,prg,expected,timeout):
 import vice_reu
 original=vice_reu.parse; vice_reu.parse=parse
 try:return run_raw(vice,prg,expected,timeout)
 finally:vice_reu.parse=original
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--vice',type=pathlib.Path,required=True);ap.add_argument('--prg',type=pathlib.Path,required=True);ap.add_argument('--timeout',type=float,default=180);ap.add_argument('--output',type=pathlib.Path);a=ap.parse_args()
 cases=[run(a.vice.resolve(strict=True),a.prg.resolve(strict=True),k,a.timeout) for k in SIZES]
 text=json.dumps({'schema':'KSA64 phase5 adaptive history REU matrix v1','cases':cases},indent=2)+'\n'
 if a.output:a.output.write_text(text,encoding='utf8')
 print(text,end='')
if __name__=='__main__':main()
