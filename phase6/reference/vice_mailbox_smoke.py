#!/usr/bin/env python3
from __future__ import annotations
import argparse, socket, struct, subprocess, sys, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import COMMAND_EXIT, ViceMonitor, available_port
BOX_MAGIC=0x36424D4B; RESULT_MAGIC=0x36464C4B
class ProvenFailure(RuntimeError): pass
def crc16(data:bytes)->int:
 c=0xffff
 for x in data:
  c^=x<<8
  for _ in range(8): c=((c<<1)^0x1021)&0xffff if c&0x8000 else (c<<1)&0xffff
 return c
def cells():
 a=bytearray(64);a[:10]=bytes((0xd6,0x5a,6,3))+struct.pack('<HHH',0x6a52,0,0);struct.pack_into('<HHii',a,10,6,0,0,0);struct.pack_into('<iii',a,22,22958965,0,12465701);struct.pack_into('<iii',a,34,0,6857499,0);struct.pack_into('<hhh',a,46,0,-8066,0);struct.pack_into('<iI',a,52,0,1);struct.pack_into('<H',a,62,crc16(a[:62]))
 i=bytearray(40);i[:10]=bytes((0xd6,0x5a,6,1))+struct.pack('<HHH',0x6a52,0,0);i[10]=0xff;i[11]=1;struct.pack_into('<hhh',i,12,0,-8066,0);struct.pack_into('<hhh',i,18,0,0,0);struct.pack_into('<hhh',i,24,1,2,3);struct.pack_into('<hhH',i,30,0,0,1);struct.pack_into('<H',i,38,crc16(i[:38]));return bytes(a),bytes(i)
def connect_forever(port:int,process:subprocess.Popen)->socket.socket:
 last=time.monotonic()
 while True:
  if process.poll() is not None: raise ProvenFailure(f'VICE exited with {process.returncode}')
  s=socket.socket();s.settimeout(2)
  try:s.connect(('127.0.0.1',port));s.settimeout(30);return s
  except OSError:s.close()
  if time.monotonic()-last>=60: print('still waiting for VICE monitor; run remains active',flush=True);last=time.monotonic()
def wait_memory(m:ViceMonitor,process:subprocess.Popen,start:int,end:int,predicate,stalled_after_ready:float|None=None):
 begun=time.monotonic();last=begun
 while True:
  if process.poll() is not None: raise ProvenFailure(f'VICE exited with {process.returncode}')
  raw=m.read_memory(start,end)
  if predicate(raw):return raw
  m.command(COMMAND_EXIT)
  if stalled_after_ready is not None and time.monotonic()-begun>stalled_after_ready:raise ProvenFailure(f'ready endpoint stalled for {stalled_after_ready}s; memory={raw.hex()}')
  if time.monotonic()-last>=60:print('still waiting; run remains active',flush=True);last=time.monotonic()
  time.sleep(.02)
def main():
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--warp',action='store_true');a=p.parse_args();vice=a.vice.resolve(strict=True);prg=a.prg.resolve(strict=True);mp=available_port();startup=None;flags=0
 if sys.platform=='win32':startup=subprocess.STARTUPINFO();startup.dwFlags|=subprocess.STARTF_USESHOWWINDOW;startup.wShowWindow=0;flags=subprocess.CREATE_NO_WINDOW
 args=[str(vice),'-default','-pal','+sound','+confirmonexit','+saveres','-minimized','-binarymonitor','-binarymonitoraddress',f'ip4://127.0.0.1:{mp}','-autostartprgmode','1','-autostart',str(prg)]

 if a.warp:args.insert(3,'-warp')
 v=subprocess.Popen(args,cwd=vice.parent,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,startupinfo=startup,creationflags=flags);clean=False;conn=None
 print(f'VICE mailbox smoke started PID {v.pid}; warp={a.warp}; startup has no timeout',flush=True)
 try:
  conn=connect_forever(mp,v);m=ViceMonitor(conn);m.command(0x81);wait_memory(m,v,0xc800,0xc803,lambda b:int.from_bytes(b,'little')==BOX_MAGIC,120)
  aid,inertial=cells();m.write_memory(0xc808,bytes((1,)));m.write_memory(0xc810,aid);m.write_memory(0xc850,inertial);m.write_memory(0xc804,bytes((1,)));m.command(COMMAND_EXIT)
  wait_memory(m,v,0xc806,0xc806,lambda b:b==bytes((1,)),120);out=m.read_memory(0xc880,0xc8c7)
  if out[:2]!=bytes((0xd6,0x5a)) or out[3]!=2 or out[24:26]!=bytes((0xd6,0x5a)) or out[27]!=4:raise ProvenFailure(f'invalid mailbox response {out.hex()}')
  raw=wait_memory(m,v,0xc000,0xc013,lambda b:int.from_bytes(b[:4],'little')==RESULT_MAGIC,120);schema,status,epochs,reserved=struct.unpack_from('<HHHH',raw,4);nav,flight=struct.unpack_from('<II',raw,12)
  if (schema,status,epochs,reserved,nav,flight)!=(1,0,1,0,3026340201,2593577103):raise ProvenFailure(f'unexpected endpoint result {raw.hex()}')
  print({'epochs':epochs,'navigation_checksum':nav,'flight_checksum':flight,'response_bytes':72});clean=True;return 0
 except KeyboardInterrupt:
  clean=True
  raise
 except Exception as e:
  print(f'PROVEN FAILURE: {e}',file=sys.stderr);clean=True;return 1
 finally:
  if conn:conn.close()
  if clean and v.poll() is None:v.terminate();v.wait(timeout=15)
  if not clean and v.poll() is None:print(f'unclassified interruption; VICE PID {v.pid} left running',file=sys.stderr)
if __name__=='__main__':raise SystemExit(main())
