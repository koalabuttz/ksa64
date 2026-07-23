#!/usr/bin/env python3
"""Validate bounded Phase 5 KPH5 replay through VIC-II screen memory."""
from __future__ import annotations
import argparse,hashlib,json,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'phase0/reference'))
from vice_timing import run_prg_until_result

def char(c):
 if 1<=c<=26:return chr(64+c)
 if 32<=c<=63:return chr(c)
 return '?'
def parse(memory):
 rows=[''.join(char(c) for c in memory[n:n+40]).rstrip() for n in range(0,1000,40)]
 if rows[24].startswith('PHASE 5 REPLAY ERROR'):raise RuntimeError(rows[24])
 if rows[24]!='PHASE 5 REPLAY PASS':return None
 expected={0:'KSA64 PHASE 5 REPLAY',1:'RUN 0000 POINTS 0099  STEP 3133',2:'POS 51E5 0EC4 3BE2',3:'MAXQ 02B9 NAV 0006  EVENTS 0007',4:'SID I02 C02 S01 A00 R00',21:'Y-Z PROJECTION QUARTER KM',22:'CUE HASH 3B2FB64B',23:'KPH5 CRC F2B3B81F',24:'PHASE 5 REPLAY PASS'}
 for n,v in expected.items():
  if rows[n]!=v:raise RuntimeError(f'row {n}: expected {v!r}, got {rows[n]!r}')
 plot=sum(1 for c in memory[5*40:21*40] if c!=32)
 return {'passed':True,'screen_sha256':hashlib.sha256(memory).hexdigest(),'plot_cells':plot,'rows':{str(n):rows[n] for n in expected}}
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--vice',type=Path,required=True);ap.add_argument('--prg',type=Path,required=True);ap.add_argument('--timeout',type=float,default=180);a=ap.parse_args()
 out=run_prg_until_result(a.vice.resolve(strict=True),a.prg.resolve(strict=True),a.timeout,0x0400,0x07e7,parse);print(json.dumps(out,indent=2))
if __name__=='__main__':main()
