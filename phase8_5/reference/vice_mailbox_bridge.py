#!/usr/bin/env python3
from __future__ import annotations
import argparse, hashlib, json, socket, struct, subprocess, sys, time, zlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'phase0'/'reference'))
from vice_timing import ViceMonitor,available_port
sys.path.insert(0,str(ROOT/'phase6'/'reference'))
from vice_mailbox_smoke import ProvenFailure,connect_forever,wait_memory
BOX_MAGIC=0x38424D4B
RESULT_MAGIC=0x38464C4B
SESSION=0x4B4C5238
NONE=0xFFFFFFFF

def cobs_encode(data:bytes)->bytes:
    out=bytearray([0]);code_at=0;code=1
    for value in data:
        if value==0:
            out[code_at]=code;code_at=len(out);out.append(0);code=1
        else:
            out.append(value);code+=1
            if code==0xFF:
                out[code_at]=code;code_at=len(out);out.append(0);code=1
    out[code_at]=code;out.append(0);return bytes(out)
def cobs_decode(data:bytes)->bytes:
    if not data or data[-1]!=0:raise ProvenFailure('unterminated KLF6 frame')
    out=bytearray();i=0;end=len(data)-1
    while i<end:
        code=data[i];i+=1
        if code==0 or i+code-1>end:raise ProvenFailure('bad COBS')
        out.extend(data[i:i+code-1]);i+=code-1
        if code!=0xFF and i<end:out.append(0)
    return bytes(out)
def make_frame(kind:int,seq:int,measurement:int,production:int,effective:int,payload:bytes)->bytes:
    decoded=bytearray(36+len(payload)+4);decoded[:4]=b'KLF6';decoded[4]=6;decoded[5]=kind
    struct.pack_into('<HIIIIIIH',decoded,6,0,SESSION,seq,NONE,measurement,production,effective,len(payload));decoded[36:36+len(payload)]=payload
    struct.pack_into('<I',decoded,36+len(payload),zlib.crc32(decoded[:36+len(payload)])&0xFFFFFFFF);return cobs_encode(decoded)
def recv_frame(wire:socket.socket):
    encoded=bytearray()
    while True:
        value=wire.recv(1)
        if not value:raise ProvenFailure('broker closed KLF6 link')
        encoded+=value
        if value==b'\0':break
    decoded=cobs_decode(bytes(encoded))
    if len(decoded)<40 or decoded[:4]!=b'KLF6' or decoded[4]!=6:raise ProvenFailure('bad KLF6 frame')
    kind=decoded[5];session,seq,ack,measurement,production,effective=struct.unpack_from('<IIIIII',decoded,8);length=struct.unpack_from('<H',decoded,32)[0]
    if session!=SESSION or len(decoded)!=40+length or zlib.crc32(decoded[:-4])&0xFFFFFFFF!=struct.unpack_from('<I',decoded,len(decoded)-4)[0]:raise ProvenFailure('corrupt KLF6 frame')
    return kind,seq,measurement,production,effective,decoded[36:36+length]
def capabilities(gimbal:bool)->bytes:
    out=bytearray(28);out[0]=2;out[1]=1;vehicle=0x85001001 if gimbal else 0xD4C509B0;struct.pack_into('<III',out,4,13,0x06010001,vehicle);struct.pack_into('<I',out,16,0x08520001);struct.pack_into('<HBBB',out,20,512,32,8,1);return bytes(out)
def main()->int:
    parser=argparse.ArgumentParser();parser.add_argument('--vice',type=Path,required=True);parser.add_argument('--prg',type=Path,required=True);parser.add_argument('--broker',type=Path,required=True);parser.add_argument('--max-releases',type=int,default=8);parser.add_argument('--display',choices=('summary','tui'),default='summary');parser.add_argument('--gimbal',action='store_true');parser.add_argument('--pace',choices=('fast','realtime'),default='fast');parser.add_argument('--output',type=Path);parser.add_argument('--vice-log',type=Path);args=parser.parse_args()
    if not 1<=args.max_releases<=65535:parser.error('max releases must be 1..65535')
    vice=args.vice.resolve(strict=True);prg=args.prg.resolve(strict=True);broker=args.broker.resolve(strict=True);monitor_port=available_port();broker_port=available_port();startup=None;flags=0
    if sys.platform=='win32':startup=subprocess.STARTUPINFO();startup.dwFlags|=subprocess.STARTF_USESHOWWINDOW;startup.wShowWindow=0;flags=subprocess.CREATE_NO_WINDOW
    vice_command=[str(vice),'-default','-pal','+sound','+confirmonexit','+saveres','-minimized','-binarymonitor','-binarymonitoraddress',f'ip4://127.0.0.1:{monitor_port}','-autostartprgmode','1','-autostart',str(prg)]
    if args.vice_log:vice_command.extend(('-logfile',str(args.vice_log.resolve())))
    vp=subprocess.Popen(vice_command,cwd=vice.parent,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,startupinfo=startup,creationflags=flags)
    bp=None;mc=None;wire=None;started=time.monotonic();print(f'VICE Phase 8.5 probe PID {vp.pid}; one instance; no warp; no time limit',flush=True)
    try:
        mc=connect_forever(monitor_port,vp);monitor=ViceMonitor(mc);monitor.command(0x81);wait_memory(monitor,vp,0xC800,0xC803,lambda b:int.from_bytes(b,'little')==BOX_MAGIC,120);print('C64 KLR8 mailbox ready',flush=True)
        broker_command=[str(broker),'--listen',f'127.0.0.1:{broker_port}','--display',args.display,'--pace',args.pace]
        if args.gimbal:broker_command.append('--gimbal')
        if args.display!='tui':broker_command.extend(('--max-releases',str(args.max_releases)))
        interactive=args.display=='tui'
        bp=subprocess.Popen(broker_command,cwd=ROOT,stdin=None if interactive else subprocess.DEVNULL,stdout=None if interactive else subprocess.PIPE,stderr=None if interactive else subprocess.PIPE,text=True,startupinfo=None if interactive else startup,creationflags=0 if interactive else flags)
        deadline=time.monotonic()+120
        while True:
            try:wire=socket.create_connection(('127.0.0.1',broker_port));break
            except OSError:
                if bp.poll() is not None:raise ProvenFailure(f'broker exited before accepting the link: {bp.returncode}')
                if time.monotonic()>=deadline:raise ProvenFailure('broker did not listen within 120 seconds')
                time.sleep(0.05)
        wire.settimeout(120);monitor.write_memory(0xC80B,bytes((1 if args.gimbal else 0,)));wire.sendall(make_frame(2,0,0,0,0,capabilities(args.gimbal)))
        if recv_frame(wire)[0]!=3:raise ProvenFailure('missing Start frame')
        sequence=0;completed=0
        while True:
            kind,seq,measurement,production,effective,payload=recv_frame(wire)
            if kind==10:
                monitor.write_memory(0xC80A,b'\x01');monitor.command(0xAA);break
            if kind!=4:raise ProvenFailure(f'unexpected world record {kind}')
            aid=None
            if len(payload)==64:
                aid=payload
                kind,seq,measurement,production,effective,payload=recv_frame(wire)
            if kind!=4 or len(payload)!=40:raise ProvenFailure('missing inertial KLR8 cell')
            epoch=struct.unpack_from('<H',payload,6)[0]
            sequence=(sequence+1)&0xFF;monitor.write_memory(0xC808,bytes((1 if aid else 0,)))
            if aid:monitor.write_memory(0xC810,aid)
            monitor.write_memory(0xC850,payload);monitor.write_memory(0xC804,bytes((sequence,)));monitor.command(0xAA)
            wait_memory(monitor,vp,0xC806,0xC806,lambda b,e=sequence:b==bytes((e,)),120)
            command=monitor.read_memory(0xC880,0xC897);wire.sendall(make_frame(5,epoch*2+1,epoch,epoch,epoch+1,command))
            present=monitor.read_memory(0xC809,0xC809)[0]
            if epoch&3==0:
                if present!=1:raise ProvenFailure(f'missing status at epoch {epoch}')
                status=monitor.read_memory(0xC898,0xC8C7);wire.sendall(make_frame(7,epoch*2+2,epoch,epoch,NONE,status))
            elif present:raise ProvenFailure(f'unexpected status at epoch {epoch}')
            monitor.write_memory(0xC807,bytes((sequence,)));completed+=1
        raw=wait_memory(monitor,vp,0xC000,0xC013,lambda b:int.from_bytes(b[:4],'little')==RESULT_MAGIC,120)
        if args.display=='tui':
            bp.wait(timeout=120)
            if bp.returncode!=0:raise ProvenFailure(f'interactive broker failed: {bp.returncode}')
        else:
            output,error=bp.communicate(timeout=120)
            expected=f'KSA64_PHASE85_BOUNDED releases={completed}' if completed==args.max_releases else 'KSA64_PHASE85_COMPLETE'
            if bp.returncode!=0 or expected not in output:raise ProvenFailure(f'broker mismatch: {output} {error}')
        raw_prg=prg.read_bytes();load=struct.unpack_from('<H',raw_prg,0)[0];end=load+len(raw_prg)-2
        result={'schema':'ksa64.phase8_5.vice-mailbox-probe-v1','capability':'two-axis-gimbal' if args.gimbal else 'monitor-only','releases':completed,'wall_seconds':time.monotonic()-started,'target':'PAL stock C64 via one pinned VICE instance, binary-monitor mailbox, KLF6 outer / KLR8 inner','artifact':{'bytes':len(raw_prg),'sha256':hashlib.sha256(raw_prg).hexdigest(),'load_address':load,'load_end_exclusive':end,'stock_fit':end<=0xC000},'result_raw':raw.hex()}
        text=json.dumps(result,indent=2)+'\n';print(text,end='');
        if args.output:args.output.write_text(text)
        return 0
    except KeyboardInterrupt:raise
    except Exception as error:print(f'PROVEN FAILURE: {error}',file=sys.stderr);return 1
    finally:
        if wire:wire.close()
        if mc:mc.close()
        if bp and bp.poll() is None:bp.terminate();bp.wait(timeout=15)
        if vp.poll() is None:vp.terminate();vp.wait(timeout=15)
if __name__=='__main__':raise SystemExit(main())
