#!/usr/bin/env python3
"""Independent exact vectors for PriorityResidualV1."""
from __future__ import annotations
import argparse,json,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];PACK=ROOT/'phase9_5'/'examples'/'firestorm-m9.kpa9';OUT=ROOT/'phase9_5'/'generated'/'allocator_vectors_v1.rs'
def i16(b,o):return struct.unpack_from('<h',b,o)[0]
def i32(b,o):return struct.unpack_from('<i',b,o)[0]
def trunc(n,d):return int(n/d)
def round_shift(v,s):return (v+(1<<(s-1)))>>s if v>=0 else -(((-v)+(1<<(s-1)))>>s)
def scale(v,s):return round_shift(v*s,15)
def ratio(v,a):return 0 if a<=0 else max(-32768,min(32767,trunc(v*(1<<15),a)))
def parse():
 b=PACK.read_bytes();auth=[[i32(b,84+(a*3+g)*4)for g in range(3)]for a in range(3)];gm=[[i16(b,128+(a*2+j)*2)for j in range(2)]for a in range(3)];cm=[[i16(b,144+(a*4+j)*2)for j in range(4)]for a in range(3)];rm=[[i16(b,176+(a*12+j)*2)for j in range(12)]for a in range(3)];return list(b[44:47]),auth,gm,cm,rm
def synth(alloc,auth,mix):
 ratios=[ratio(alloc[a],auth[a])for a in range(3)];return[max(-32768,min(32767,round_shift(sum(ratios[a]*mix[a][j]for a in range(3)),15)))for j in range(len(mix[0]))]
def pred_cont(actual,auth,mix):
 out=[]
 for a in range(3):
  norm=sum(x*x for x in mix[a]);dot=sum(actual[j]*mix[a][j]for j in range(len(actual)));r=0 if not norm else max(-32768,min(32767,trunc(dot*(1<<15),norm)));out.append(scale(auth[a],r))
 return out
def continuous(res,auth,mix,limits):
 alloc=[max(-auth[a],min(auth[a],res[a]))for a in range(3)];norm=synth(alloc,auth,mix);cmd=[max(-limits[j],min(limits[j],scale(limits[j],norm[j])))for j in range(len(limits))];actual=[ratio(cmd[j],limits[j])for j in range(len(limits))];got=pred_cont(actual,auth,mix);return got,cmd,int(got!=alloc)
def pred_rcs(actual,auth,mix):
 out=[]
 for a in range(3):
  num=sum(actual[j]*mix[a][j]for j in range(12));den=sum(x for x in mix[a]if x>0);r=0 if not den else max(-32768,min(32767,trunc(num,den)));out.append(scale(auth[a],r))
 return out
def rcs(res,auth,mix):
 alloc=[max(-auth[a],min(auth[a],res[a]))for a in range(3)];norm=synth(alloc,auth,mix);pulse=[max(0,min(8,(max(0,v)*8)>>15))for v in norm];actual=[min(32767,q*32768//8)for q in pulse];got=pred_rcs(actual,auth,mix);return got,pulse,int(got!=alloc)
def run(demand,groups):
 priorities,aa,gm,cm,rm=parse();res=list(demand);got=[0,0,0];gimbal=[0,0];canards=[0]*4;pulse=[0]*12;sat=0
 for group in priorities:
  if group not in groups:continue
  auth=[aa[a][group-1]for a in range(3)]
  if group==1:a,gimbal,s=continuous(res,auth,gm,[910,910])
  elif group==2:a,canards,s=continuous(res,auth,cm,[1820]*4)
  else:a,pulse,s=rcs(res,auth,rm)
  got=[got[i]+a[i]for i in range(3)];res=[demand[i]-got[i]for i in range(3)];sat+=s
 if res!=[0,0,0]:sat+=1
 return {'gimbal':gimbal,'canards':canards,'pulses':pulse,'achieved':got,'residual':res,'saturation':sat}
def fixture_signature(cases):
 h=0x0950a110
 def add(v):
  nonlocal h
  h=(((h<<5)|(h>>27))+(v&0xffffffff))&0xffffffff
 for name in ('GIMBAL','CANARD','RCS','MIXED'):
  c=cases[name]
  for key in ('gimbal','canards','pulses','achieved','residual'):
   for value in c[key]:add(value)
  add(c['saturation'])
 return h
def arr(x):return '['+', '.join(str(v)for v in x)+']'
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--check',action='store_true');ap.add_argument('--report',action='store_true');a=ap.parse_args();cases={'GIMBAL':run([0,1000,-500],{1}),'CANARD':run([1000,1000,-1000],{2}),'RCS':run([1000,1000,-1000],{3}),'MIXED':run([4000,7000,-6500],{1,2,3})};text='// Generated independently by phase9_5/reference/generate_allocator_vectors.py. Do not edit.\n'
 text+=f'pub const ALLOCATOR_SIGNATURE: u32 = 0x{fixture_signature(cases):08x};\n'
 for name,c in cases.items():
  text+=f'pub const {name}_GIMBAL: [i16; 2] = {arr(c["gimbal"])};\n';text+=f'pub const {name}_CANARDS: [i16; 4] = {arr(c["canards"])};\n';text+=f'pub const {name}_PULSES: [u8; 12] = {arr(c["pulses"])};\n';text+=f'pub const {name}_ACHIEVED: [i32; 3] = {arr(c["achieved"])};\n';text+=f'pub const {name}_RESIDUAL: [i32; 3] = {arr(c["residual"])};\n';text+=f'pub const {name}_SATURATION: u16 = {c["saturation"]};\n'
 if a.check:
  if not OUT.exists()or OUT.read_text()!=text:raise SystemExit('allocator vectors stale')
 else:OUT.write_text(text)
 if a.report:print(json.dumps(cases,sort_keys=True))
 return 0
if __name__=='__main__':raise SystemExit(main())
