#!/usr/bin/env python3
"""Independent strict KST5 byte-stream verifier and evidence freezer."""
from __future__ import annotations
import argparse,hashlib,json,struct,zlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
EVIDENCE=ROOT/'phase5'/'telemetry-reference-v1.json'
SHA=EVIDENCE.with_suffix(EVIDENCE.suffix+'.sha256')
H=96;F=424

def u16(b,o):return struct.unpack_from('<H',b,o)[0]
def u32(b,o):return struct.unpack_from('<I',b,o)[0]
def i32(b,o):return struct.unpack_from('<i',b,o)[0]
def crc(b):return zlib.crc32(b)&0xffffffff
def fnv(h,b):
 for x in b:h=((h^x)*16777619)&0xffffffff
 return h

def verify(path:Path):
 data=path.read_bytes();assert len(data)>=H and (len(data)-H)%F==0
 h=data[:H];assert h[:4]==b'KST5';assert (u16(h,4),u16(h,6),u16(h,8),u16(h,10))==(5,96,424,1)
 assert (u32(h,12),u32(h,16),u32(h,20),u32(h,24),u32(h,28))==(0x05000001,1560508115,4178197782,2082184392,1563829522)
 assert h[37:39]==bytes((1,1)) and h[39]==0 and not any(h[60:92]);assert i32(h,40)==8192 and u32(h,44)==7200 and crc(h[:92])==u32(h,92)
 assert u32(h,32)==0x5a000000|h[36]
 count=(len(data)-H)//F;chain=2166136261;event_frames=0;last=None
 for n in range(count):
  f=data[H+n*F:H+(n+1)*F];assert crc(f[:420])==u32(f,420);assert not any(f[416:420]);assert u32(f,0)==n;assert i32(f,4)==8192*n
  flags=u16(f,8);events=u16(f,10);assert flags&~3==0 and events&~0x0fff==0;assert bool(flags&2)==bool(events);assert bool(flags&1)==(n+1==count)
  sensor=f[176:304];command=f[368:400];assert crc(sensor[:124])==u32(sensor,124);assert crc(command[:28])==u32(command,28)
  expected=0 if n==0 else n-1;assert u32(sensor,0)==expected==u32(f,304)==u32(command,0);assert u16(f,18)==u16(sensor,8)
  chain=fnv(chain,f[:412]);assert u32(f,412)==chain;event_frames+=bool(flags&2);last=f
 assert last is not None
 result={'format':'KST5','version':1,'stream_bytes':len(data),'stream_sha256':hashlib.sha256(data).hexdigest(),'stream_crc32':f'0x{crc(data):08x}','frame_count':count,'event_frames':event_frames,'header_crc32':f'0x{u32(h,92):08x}','case':h[36],'seed':f'0x{u32(h,32):08x}','final':{'step':u32(last,0),'position_q12':[i32(last,44+4*k) for k in range(3)],'velocity_q24':[i32(last,56+4*k) for k in range(3)],'sensor_checksum':f'0x{u32(last,400):08x}','navigation_checksum':f'0x{u32(last,404):08x}','flight_checksum':f'0x{u32(last,408):08x}','observation_checksum':f'0x{u32(last,412):08x}'}}
 return result

def main():
 ap=argparse.ArgumentParser();ap.add_argument('stream',type=Path);ap.add_argument('--update',action='store_true');ap.add_argument('--check',action='store_true');a=ap.parse_args();got=verify(a.stream)
 text=json.dumps(got,indent=2)+'\n'
 if a.update:EVIDENCE.write_text(text,encoding='utf-8');SHA.write_text(hashlib.sha256(text.encode()).hexdigest()+'  '+EVIDENCE.name+'\n',encoding='utf-8')
 if a.check:
  assert EVIDENCE.read_text(encoding='utf-8')==text;expected=SHA.read_text().split()[0];assert hashlib.sha256(text.encode()).hexdigest()==expected
 print(f"KST5 independent verification passed: {got['frame_count']} frames, {got['stream_crc32']}")
if __name__=='__main__':main()