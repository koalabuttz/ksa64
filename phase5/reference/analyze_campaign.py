#!/usr/bin/env python3
"""Independent KSC5/KSR5 campaign reconstruction and float64 orbit audit."""
from __future__ import annotations
import argparse,hashlib,json,math,struct,zlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];ER=6378.137;MU=398600.4418

def u16(b,o):return struct.unpack_from('<H',b,o)[0]
def u32(b,o):return struct.unpack_from('<I',b,o)[0]
def i32(b,o):return struct.unpack_from('<i',b,o)[0]
def crc(b):return zlib.crc32(b)&0xffffffff
def mix(v):
 v^=v>>16;v=(v*0x7feb352d)&0xffffffff;v^=v>>15;v=(v*0x846ca68b)&0xffffffff;return v^(v>>16)
def high(a,b):return (a*b)>>32
def keyed(seed,run,param,group,draw):return mix(seed^((run*0x9e3779b9)&0xffffffff)^(((param if group==0 else 0x100+group)*0x85ebca6b)&0xffffffff)^((draw*0xc2b2ae35)&0xffffffff))
def uniform(word,lo,hi):return lo+high(word,hi-lo+1)
def sample(s,seed,run):
 p,k,g,lo,base,hi,shape=s
 if run==0 or k==0:return base
 w=lambda n:keyed(seed,run,p,g,n)
 if k==1:return uniform(w(0),lo,hi)
 if k==2:return int((uniform(w(0),lo,hi)+uniform(w(1),lo,hi))/2)
 if k==3:return hi if high(w(0),1_000_000)<shape else lo
 total=sum(w(n)&255 for n in range(12));center=total-1530;span=hi-base if center>=0 else base-lo;return max(lo,min(hi,base+int(center*span/768)))
def fnv(h,b):
 for x in b:h=((h^x)*16777619)&0xffffffff
 return h
def orbit(pos,vel):
 r=[x/4096 for x in pos];v=[x/2**24 for x in vel];rm=math.sqrt(sum(x*x for x in r));v2=sum(x*x for x in v);h=[r[1]*v[2]-r[2]*v[1],r[2]*v[0]-r[0]*v[2],r[0]*v[1]-r[1]*v[0]];hm=math.sqrt(sum(x*x for x in h));e0=.5*v2-MU/rm
 if e0>=0 or hm==0:return None
 ecc=math.sqrt(max(0,1+2*e0*hm*hm/(MU*MU)));a=-MU/(2*e0);return {'perigee_km':a*(1-ecc)-ER,'apogee_km':a*(1+ecc)-ER,'inclination_deg':math.degrees(math.acos(max(-1,min(1,h[2]/hm))))}

def analyze(ksc_path,ksr_path):
 c=ksc_path.read_bytes();r=ksr_path.read_bytes();assert len(c)==704 and c[:4]==b'KSC5' and u16(c,4)==5 and u16(c,6)==704 and u32(c,8)==0x050a0000;assert u32(c,12)==1560508115 and u32(c,16)==4178197782;assert c[29:31]==bytes((15,24)) and not any(c[31:120]);assert crc(c[128:])==u32(c,120) and crc(c[:124])==u32(c,124)
 seed=u32(c,20);runs=u32(c,24);count=c[28];assert len(r)==runs*160
 specs=[]
 for n in range(24):
  q=128+n*24
  if n<count:assert c[q+3]==0 and crc(c[q:q+20])==u32(c,q+20);specs.append((c[q],c[q+1],c[q+2],i32(c,q+4),i32(c,q+8),i32(c,q+12),i32(c,q+16)))
  else:assert not any(c[q:q+24])
 outcomes=[0]*5;chain=2166136261;orbits=[];max_q=[];nav=[]
 for n in range(runs):
  b=r[n*160:(n+1)*160];assert b[:4]==b'KSR5' and u16(b,4)==5 and u16(b,6)==160 and u32(b,8)==0x050a0001 and u32(b,12)==seed and u32(b,16)==n;assert not any(b[30:32]) and not any(b[104:156]) and crc(b[:156])==u32(b,156)
  values=[0]*15
  for s in specs:values[s[0]]=sample(s,seed,n)-s[4]
  sensor_seed=0x5a000000 if n==0 else (mix(seed^((n*0xd1b54a35)&0xffffffff)^0x53454544) or 0x6d2b79f5)
  raw=struct.pack('<II',n,sensor_seed)+b''.join(struct.pack('<i',x) for x in values);assert sensor_seed==u32(b,20) and crc(raw)==u32(b,24)
  outcomes[b[28]]+=1;chain=fnv(chain,b);max_q.append(i32(b,72)/65536);nav.append(i32(b,84)/4096)
  o=orbit([i32(b,36+4*k) for k in range(3)],[i32(b,48+4*k) for k in range(3)]) if b[28] in (0,1) else None
  if o:orbits.append(o)
 result={'format':'KSC5/KSR5','runs':runs,'master_seed':f'0x{seed:08x}','config_crc32':f'0x{crc(c):08x}','config_sha256':hashlib.sha256(c).hexdigest(),'summary_sha256':hashlib.sha256(r).hexdigest(),'ordered_summary_chain':f'0x{chain:08x}','outcomes':{'stable_orbit':outcomes[0],'complete_not_orbit':outcomes[1],'aborted':outcomes[2],'numeric_fault':outcomes[3],'step_limit':outcomes[4]},'authoritative_float64':{'propagatable_runs':len(orbits),'perigee_range_km':[min(x['perigee_km'] for x in orbits),max(x['perigee_km'] for x in orbits)],'apogee_range_km':[min(x['apogee_km'] for x in orbits),max(x['apogee_km'] for x in orbits)],'inclination_range_deg':[min(x['inclination_deg'] for x in orbits),max(x['inclination_deg'] for x in orbits)],'stable_perigee_at_least_120_km':sum(x['perigee_km']>=120 for x in orbits)},'loads':{'max_dynamic_pressure_kpa_range':[min(max_q),max(max_q)],'max_navigation_error_km':max(nav)}}
 return result

def main():
 a=argparse.ArgumentParser();a.add_argument('--ksc',type=Path,required=True);a.add_argument('--ksr',type=Path,required=True);a.add_argument('--output',type=Path,required=True);a.add_argument('--check',action='store_true');x=a.parse_args();text=json.dumps(analyze(x.ksc,x.ksr),indent=2)+'\n';sha=x.output.with_name(x.output.name+'.sha256')
 if x.check:assert x.output.read_text()==text;assert sha.read_text().split()[0]==hashlib.sha256(text.encode()).hexdigest()
 else:x.output.write_text(text);sha.write_text(hashlib.sha256(text.encode()).hexdigest()+'  '+x.output.name+'\n')
 print(f"Phase 5 campaign analysis passed: {json.loads(text)['runs']} runs")
if __name__=='__main__':main()