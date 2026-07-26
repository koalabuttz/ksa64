#!/usr/bin/env python3
from pathlib import Path
import argparse,json,struct,sys
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import run_prg_until_result
MAGIC=0x3942544D
def parse(b):
 if struct.unpack_from('<I',b,0)[0]!=MAGIC:return None
 status,cycles=struct.unpack_from('<II',b,4);return {'status':status,'cycles':cycles}
def main():
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--output',type=Path);a=p.parse_args();samples=[run_prg_until_result(a.vice.resolve(),a.prg.resolve(),120,0xc000,0xc00b,parse) for _ in range(3)];assert samples[0]==samples[1]==samples[2];data={'schema':'ksa64.phase9_5.advanced-wrapper-timing-v1','samples':samples,'cycles':samples[0]['cycles']};print(json.dumps(data,indent=2));a.output and a.output.write_text(json.dumps(data,indent=2)+'\n')
if __name__=='__main__':main()
