#!/usr/bin/env python3
from __future__ import annotations
import argparse,hashlib,json,struct,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'phase0'/'reference'));from vice_timing import run_prg_until_result
MAGIC=0x3645504B
def parse(b:bytes):
 if struct.unpack_from('<I',b,0)[0]!=MAGIC:return None
 schema,status,count,reserved=struct.unpack_from('<HHHH',b,4)
 if schema!=1 or status or reserved:raise RuntimeError(f'endpoint probe failure schema={schema} status={status} reserved={reserved}')
 h,n,f,g=struct.unpack_from('<IIII',b,12);return {'command_bytes':count,'wire_hash':h,'navigation_checksum':n,'flight_checksum':f,'guidance_signature':g}
def main():
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--runs',type=int,default=3);p.add_argument('--output',type=Path);a=p.parse_args();samples=[run_prg_until_result(a.vice.resolve(strict=True),a.prg.resolve(strict=True),120,0xc000,0xc01b,parse)for _ in range(a.runs)]
 if len({json.dumps(s,sort_keys=True)for s in samples})!=1:raise RuntimeError('nondeterministic endpoint probe')
 raw=a.prg.read_bytes();load=struct.unpack_from('<H',raw,0)[0];end=load+len(raw)-2;data={'schema':'ksa64.phase6.endpoint-probe-v1','runs':a.runs,'result':samples[0],'artifact':{'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'load_address':load,'load_end_exclusive':end,'stock_fit':end<=0xc000}}
 text=json.dumps(data,indent=2)+'\n';print(text,end='');
 if a.output:a.output.write_text(text)
 return 0
if __name__=='__main__':raise SystemExit(main())
