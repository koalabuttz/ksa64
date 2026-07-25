#!/usr/bin/env python3
"""Independent strict validator for Phase 8 KSC8/KRA8 campaigns."""
from __future__ import annotations
import argparse,binascii,hashlib,json,struct
from collections import Counter
from pathlib import Path
KSC=512;KSR=256;KRAH=64;STRIDE=264;SEED=0x4B534138
RANGES=[(-10000,10000),(-20000,20000),(-50000,50000),(-50000,50000),(-1449552,1449552),(-20000,20000),(0,2<<22),(0,65535),(0,1<<20),(-2342321,2342321),(-50000,50000),(-50000,50000)]
def u32(b,o):return struct.unpack_from('<I',b,o)[0]
def i32(b,o):return struct.unpack_from('<i',b,o)[0]
def crc(b):return binascii.crc32(b)&0xffffffff
def validate_record(b,magic,length):
 if len(b)!=length or b[:4]!=magic or struct.unpack_from('<H',b,4)[0]!=8 or struct.unpack_from('<H',b,6)[0]!=32 or struct.unpack_from('<H',b,8)[0]!=length or u32(b,len(b)-4)!=crc(b[:-4]):raise ValueError(f'invalid {magic.decode()} record')
def mix32(v):
 v^=v>>16;v=(v*0x7feb352d)&0xffffffff;v^=v>>15;v=(v*0x846ca68b)&0xffffffff;return (v^(v>>16))&0xffffffff
def keyed(seed,run,param):return mix32(seed^((run*0x9e3779b9)&0xffffffff)^((param*0x85ebca6b)&0xffffffff))
def variations(seed,run):
 if run==0:return [0]*12,0
 out=[]
 for p,(lo,hi) in enumerate(RANGES):out.append(lo+((keyed(seed,run,p)*(hi-lo+1))>>32))
 raw=b''.join(struct.pack('<i',v)for v in out);return out,crc(raw)
def fnv(words):
 h=2166136261
 for word in words:
  for byte in struct.pack('<I',word):h=((h^byte)*16777619)&0xffffffff
 return h
def analyze(ksc_path,kra_path):
 c=ksc_path.read_bytes();validate_record(c,b'KSC8',KSC);seed,runs,catalog=u32(c,32),u32(c,36),u32(c,40)
 if seed!=SEED or catalog!=0x08000001 or c[44]!=len(RANGES):raise ValueError('campaign identity')
 for p,(lo,hi) in enumerate(RANGES):
  o=48+p*16
  if c[o]!=p or c[o+1]!=1 or i32(c,o+4)!=lo or i32(c,o+8)!=hi or any(c[o+12:o+16]):raise ValueError(f'catalog {p}')
 a=kra_path.read_bytes();validate_record(a[:KRAH],b'KRA8',KRAH)
 if len(a)!=KRAH+runs*STRIDE or u32(a,40)!=STRIDE:raise ValueError('archive length')
 payload=[];outcomes=Counter();apogees=[];landings=[]
 for run in range(runs):
  o=KRAH+run*STRIDE
  if u32(a,o)!=run or u32(a,o+4)!=KSR:raise ValueError(f'order {run}')
  r=a[o+8:o+8+KSR];validate_record(r,b'KSR8',KSR)
  if r[32]!=4:raise ValueError(f'profile {run}')
  _,expected=variations(seed,run)
  if u32(r,232)!=expected:raise ValueError(f'variation {run}')
  identities=[u32(r,200+i*4)for i in range(6)];checksums=[u32(r,224+i*4)for i in range(5)]
  if u32(r,16)!=fnv(identities+checksums):raise ValueError(f'identity {run}')
  outcomes[r[33]]+=1
  validity=u32(r,40)
  if validity&(1<<1):apogees.append(i32(r,68+4))
  if validity&(1<<28):landings.append(i32(r,68+28*4))
  payload.append(r)
 payload_bytes=b''.join(payload)
 if crc(payload_bytes)!=u32(a,44):raise ValueError('ordered CRC')
 return {'schema':'ksa64.phase8-campaign-analysis-v1','seed':seed,'runs':runs,'archive_sha256':hashlib.sha256(a).hexdigest(),'ordered_summary_crc32':f'0x{crc(payload_bytes):08x}','outcomes':{str(k):v for k,v in sorted(outcomes.items())},'apogee_raw_q13':{'minimum':min(apogees,default=0),'maximum':max(apogees,default=0),'mean':sum(apogees)//len(apogees) if apogees else 0},'maximum_landing_distance_raw_q13':max(landings,default=0)}
def main():
 p=argparse.ArgumentParser();p.add_argument('--ksc',type=Path,required=True);p.add_argument('--kra',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--check',action='store_true');args=p.parse_args();render=json.dumps(analyze(args.ksc,args.kra),indent=2,sort_keys=True)+'\n'
 if args.check:
  if args.output.read_text()!=render:raise SystemExit('stale campaign analysis')
 else:args.output.write_text(render,newline='\n')
 print(render,end='')
if __name__=='__main__':main()
