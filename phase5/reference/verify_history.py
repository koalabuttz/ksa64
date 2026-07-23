#!/usr/bin/env python3
"""Independent strict KPH5 reader and Phase 5 adaptive-storage evidence freezer."""
from __future__ import annotations
import argparse, hashlib, json, pathlib, struct, zlib
ROOT=pathlib.Path(__file__).resolve().parents[2]
KPH=ROOT/'phase5/examples/ksa5-baseline.kph5'
KSR=ROOT/'phase5/examples/ksa5-reference.ksr5'
OUT=ROOT/'phase5/history-evidence-v1.json'
CAPS=[128,256,512,1024,2048,4096,8192,16384]

def u16(b,o): return struct.unpack_from('<H',b,o)[0]
def i16(b,o): return struct.unpack_from('<h',b,o)[0]
def u32(b,o): return struct.unpack_from('<I',b,o)[0]
def i32(b,o): return struct.unpack_from('<i',b,o)[0]
def crc(b): return zlib.crc32(b)&0xffffffff

def read_kph(path):
 d=path.read_bytes(); assert len(d)>=80; h=d[:80]
 assert h[:4]==b'KPH5' and (u16(h,4),u16(h,6),u16(h,8))==(5,80,16)
 assert u32(h,12)==0x050c0001 and u32(h,16)==0x5d0376d3 and u32(h,20)==0xf90a3d16
 assert not any(h[42:44]+h[48:72]); assert u32(h,76)==crc(h[:76])
 count=u16(h,40); assert len(d)==80+count*16; assert u32(h,72)==crc(d[80:])
 pts=[]
 for n in range(count):
  p=d[80+n*16:96+n*16]
  pts.append({'step':u16(p,0),'position_quarter_km':[i16(p,2),i16(p,4),i16(p,6)],'dynamic_pressure_sixteenth_kpa':u16(p,8),'navigation_error_quarter_km':u16(p,10),'events':u16(p,12),'alarms':u16(p,14)})
 assert pts[0]['step']==0 and pts[-1]['step']==u32(h,44)
 assert all(pts[n-1]['step']<pts[n]['step'] for n in range(1,len(pts)))
 return d,h,pts

def read_summaries(path):
 d=path.read_bytes(); assert len(d)%160==0; out=[]
 for n in range(len(d)//160):
  b=d[n*160:(n+1)*160]; assert b[:4]==b'KSR5' and u16(b,4)==5 and u16(b,6)==160 and u32(b,156)==crc(b[:156]); assert u32(b,16)==n
  out.append({'run':n,'outcome':b[28],'perigee':i32(b,60),'max_q':i32(b,72),'nav':i32(b,84)})
 return out

def select(s):
 candidates=[s[0],min(s,key=lambda x:(x['perigee'],x['run'])),max(s,key=lambda x:(x['max_q'],-x['run'])),max(s,key=lambda x:(x['nav'],-x['run'])),next((x for x in s if x['outcome']!=0),None)]
 result=[]
 for x in candidates+s:
  if x is not None and x['run'] not in result: result.append(x['run'])
 return result

def plan(kib):
 run_count=256; points=393; frames=3134; capacity=kib*1024
 summaries=min(run_count,(capacity//4)//160); used=256+128+summaries*160+3*32
 full_cost=96+frames*424+32; compact_cost=80+points*16+32
 full=min(run_count,(capacity-used)//full_cost); used+=full*full_cost
 compact=min(run_count-full,(capacity-used)//compact_cost); used+=compact*compact_cost
 return {'capacity_kib':kib,'summary_slots':summaries,'full_histories':full,'compact_histories':compact,'used_bytes':used,'free_bytes':capacity-used}

def evidence(kph=KPH):
 d,h,p=read_kph(kph); s=read_summaries(KSR); chosen=select(s)
 return {'format':'KPH5','version':5,'contract_id':'0x050c0001','stream_bytes':len(d),'stream_sha256':hashlib.sha256(d).hexdigest(),'stream_crc32':f'0x{crc(d):08x}','campaign_seed':f'0x{u32(h,24):08x}','run_index':u32(h,28),'sensor_seed':f'0x{u32(h,32):08x}','variation_checksum':f'0x{u32(h,36):08x}','stride':u16(h,10),'point_count':len(p),'terminal_step':u32(h,44),'points_crc32':f'0x{u32(h,72):08x}','first_point':p[0],'last_point':p[-1],'max_dynamic_pressure_sixteenth_kpa':max(x['dynamic_pressure_sixteenth_kpa'] for x in p),'max_navigation_error_quarter_km':max(x['navigation_error_quarter_km'] for x in p),'event_union':f"0x{sum(set()):04x}" if False else f"0x{__import__('functools').reduce(lambda a,x:a|x['events'],p,0):04x}",'alarm_union':f"0x{__import__('functools').reduce(lambda a,x:a|x['alarms'],p,0):04x}",'stock':{'summary_slots':5,'compact_histories':1,'retained_run_indices':chosen[:5]},'selection_prefix':chosen[:12],'reu_matrix':[plan(k) for k in CAPS]}

def main():
 ap=argparse.ArgumentParser(); ap.add_argument('--input',type=pathlib.Path,default=KPH); ap.add_argument('--check',action='store_true'); args=ap.parse_args(); got=evidence(args.input); text=json.dumps(got,indent=2)+'\n'
 if args.check:
  assert OUT.read_text(encoding='utf8')==text, 'history evidence drift'; print(f"KPH5 independent verification passed: {got['point_count']} points, {got['stream_crc32']}")
 else:
  OUT.write_text(text,encoding='utf8'); print(OUT)
if __name__=='__main__': main()
