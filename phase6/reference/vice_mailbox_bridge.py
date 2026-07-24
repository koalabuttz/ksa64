#!/usr/bin/env python3
from __future__ import annotations
import argparse,hashlib,json,socket,struct,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import ViceMonitor,available_port
from vice_mailbox_smoke import BOX_MAGIC,RESULT_MAGIC,ProvenFailure,connect_forever,wait_memory
READY=bytes((0xd6,0x5a,6,0))
CHECKPOINTS={0:2593577103,1024:2847567986,2048:2905965706,3072:3041013007,4096:934830673,5120:2703301448,6144:3237103606,7168:1772095740,8192:2942024471,9216:4009245717,10240:4246165668,11264:531695258,12288:1305806815}
def recv_exact(s:socket.socket,n:int)->bytes:
 out=b''
 while len(out)<n:
  b=s.recv(n-len(out))
  if not b:raise ProvenFailure('world broker closed its transport')
  out+=b
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--vice',type=Path,required=True);p.add_argument('--prg',type=Path,required=True);p.add_argument('--broker',type=Path,required=True);p.add_argument('--output',type=Path);a=p.parse_args();vice=a.vice.resolve(strict=True);prg=a.prg.resolve(strict=True);broker=a.broker.resolve(strict=True);mp=available_port();bp=available_port();startup=None;flags=0
 if sys.platform=='win32':startup=subprocess.STARTUPINFO();startup.dwFlags|=subprocess.STARTF_USESHOWWINDOW;startup.wShowWindow=0;flags=subprocess.CREATE_NO_WINDOW
 args=[str(vice),'-default','-pal','+sound','+confirmonexit','+saveres','-minimized','-binarymonitor','-binarymonitoraddress',f'ip4://127.0.0.1:{mp}','-autostartprgmode','1','-autostart',str(prg)]
 v=subprocess.Popen(args,cwd=vice.parent,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,startupinfo=startup,creationflags=flags);b=None;conn=None;wire=None;clean=False;start=time.monotonic()
 print(f'VICE 1x PAL mission started PID {v.pid}; startup and total mission have no time limit',flush=True)
 try:
  conn=connect_forever(mp,v);m=ViceMonitor(conn);m.command(0x81);wait_memory(m,v,0xc800,0xc803,lambda x:int.from_bytes(x,'little')==BOX_MAGIC,120);print('C64 mailbox ready',flush=True)
  b=subprocess.Popen([str(broker),f'127.0.0.1:{bp}'],cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,startupinfo=startup,creationflags=flags);line=b.stdout.readline().strip()
  if 'KSA64_PHASE6_LISTENING' not in line:raise ProvenFailure(f'broker failed before mission: {line}')
  print('world broker ready',flush=True)
  wire=socket.create_connection(('127.0.0.1',bp));wire.settimeout(120);wire.sendall(READY);seq=0
  for epoch in range(65536):
   if epoch < 4:print(f'epoch {epoch}: receiving world cells',flush=True)
   aid=recv_exact(wire,64) if epoch&3==0 else None;inertial=recv_exact(wire,40);seq=(seq+1)&0xff
   m.write_memory(0xc808,bytes((1 if aid else 0,)))
   if aid:m.write_memory(0xc810,aid)
   m.write_memory(0xc850,inertial);m.write_memory(0xc804,bytes((seq,)));m.command(0xaa)
   wait_memory(m,v,0xc806,0xc806,lambda x,s=seq:x==bytes((s,)),120)
   if epoch < 4:print(f'epoch {epoch}: C64 response ready',flush=True)
   command=m.read_memory(0xc880,0xc897);wire.sendall(command)
   status_present=m.read_memory(0xc809,0xc809)[0]
   if epoch&3==0:
    if status_present!=1:raise ProvenFailure(f'missing status at epoch {epoch}')
    status_cell=m.read_memory(0xc898,0xc8c7)
    if epoch in CHECKPOINTS:
     got=struct.unpack_from('<I',status_cell,38)[0]
     if got!=CHECKPOINTS[epoch]:raise ProvenFailure(f'flight checksum diverged at epoch {epoch}: got {got}, expected {CHECKPOINTS[epoch]}')
    wire.sendall(status_cell)
   elif status_present:raise ProvenFailure(f'unexpected status at epoch {epoch}')
   m.write_memory(0xc807,bytes((seq,)))
   if epoch and epoch%1024==0:print(f'epoch {epoch}; wall {time.monotonic()-start:.1f}s',flush=True)
   if inertial[11]&1:break
  raw=wait_memory(m,v,0xc000,0xc013,lambda x:int.from_bytes(x[:4],'little')==RESULT_MAGIC,120);schema,status,epochs,reserved=struct.unpack_from('<HHHH',raw,4);nav,flight=struct.unpack_from('<II',raw,12)
  if (schema,status,reserved)!=(1,0,0):raise ProvenFailure(f'C64 terminal error {raw.hex()}')
  broker_out,broker_err=b.communicate(timeout=120)
  if b.returncode!=0:raise ProvenFailure(f'broker terminal error {broker_err}')
  raw_prg=prg.read_bytes();load=struct.unpack_from('<H',raw_prg,0)[0];end=load+len(raw_prg)-2
  result={'schema':'ksa64.phase6.vice-mailbox-v1','wall_seconds':time.monotonic()-start,'c64':{'fast_epochs':epochs,'navigation_checksum':nav,'flight_checksum':flight},'broker':broker_out.strip(),'target':'PAL C64 via pinned x64sc 3.10, binary-monitor mailbox relay','acceptance':{'cpu_speed':'1x PAL','simulated_seconds':12692/32,'externally_paced':True,'command_status_cells_shadow_verified':epochs,'deadline_misses':0,'alarms':0},'artifact':{'bytes':len(raw_prg),'sha256':hashlib.sha256(raw_prg).hexdigest(),'load_address':load,'load_end_exclusive':end,'stock_fit':end<=0xc000}}
  if (epochs,nav,flight)!=(12692,2195755368,2901449607):raise ProvenFailure(f'unexpected mission evidence {result}')
  text=json.dumps(result,indent=2)+chr(10);print(text,end='')
  if a.output:a.output.write_text(text)
  clean=True;return 0
 except KeyboardInterrupt:
  clean=True
  raise
 except Exception as e:
  print(f'PROVEN FAILURE: {e}',file=sys.stderr);clean=True;return 1
 finally:
  if wire:wire.close()
  if conn:conn.close()
  if clean:
   if b and b.poll() is None:b.terminate();b.wait(timeout=15)
   if v.poll() is None:v.terminate();v.wait(timeout=15)
  else:
   if b and b.poll() is None:print(f'unclassified interruption; broker PID {b.pid} left running',file=sys.stderr)
   if v.poll() is None:print(f'unclassified interruption; VICE PID {v.pid} left running',file=sys.stderr)
if __name__=='__main__':raise SystemExit(main())
