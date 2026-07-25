#!/usr/bin/env python3
"""Generate independent Phase 9.5 fixed-format vectors."""
from __future__ import annotations
import binascii, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'phase9_5'/'generated'/'contract_vectors_v1.rs'

def p16(b,o,v): b[o:o+2]=int(v&0xffff).to_bytes(2,'little')
def p32(b,o,v): b[o:o+4]=int(v&0xffffffff).to_bytes(4,'little')
def crc16(data):
 c=0xffff
 for byte in data:
  c^=byte<<8
  for _ in range(8): c=((c<<1)^0x1021)&0xffff if c&0x8000 else (c<<1)&0xffff
 return c

def prefix(b,kind,session,a,z): b[0:2]=bytes([0xd9,0x5a]);b[2]=9;b[3]=kind;p16(b,4,session);p16(b,6,a);p16(b,8,z)
def fast():
 b=bytearray(64);prefix(b,1,1,2,3);p16(b,10,63)
 for i,v in enumerate([1,2,3]):p16(b,12+i*2,v)
 for i,v in enumerate([4,5,6]):p16(b,18+i*2,v)
 for i,v in enumerate([7,8,9]):p16(b,24+i*2,v)
 p32(b,30,10);p16(b,34,11)
 for i,v in enumerate([12,13]):p16(b,36+i*2,v)
 for i,v in enumerate([14,15,16,17]):p16(b,40+i*2,v)
 p16(b,48,0x555);p32(b,50,18);p16(b,54,19);p16(b,56,20);p16(b,58,21);p16(b,60,22);p16(b,62,crc16(b[:62]));return b
def command():
 b=bytearray(64);prefix(b,2,1,2,3);b[10]=0;b[11]=3
 for i,v in enumerate([1,2]):p16(b,12+i*2,v)
 for i,v in enumerate([3,4,5,6]):p16(b,16+i*2,v)
 for i,v in enumerate([7,8,9]):p32(b,24+i*4,v)
 b[36:48]=bytes([1]*12);p16(b,48,10);b[50]=2;p32(b,52,11);p16(b,62,crc16(b[:62]));return b
def kle():
 b=bytearray(256);b[:4]=b'KLE9';p16(b,4,9);p16(b,6,32);p16(b,8,256);p16(b,10,3);p32(b,12,0x09500001);p32(b,16,8);b[32]=4;b[33]=1
 for o,v in zip(range(36,76,4),[2,3,4,5,6,7,1,6,0,9]):p32(b,o,v)
 p32(b,252,binascii.crc32(b[:252])&0xffffffff);return b
def arr(name,b):
 rows=[]
 for i in range(0,len(b),16): rows.append('    '+', '.join(str(x) for x in b[i:i+16])+',')
 return f'pub const {name}: [u8; {len(b)}] = [\n'+'\n'.join(rows)+'\n];\n'
def signature():
 data=bytearray()
 for value in [0xc07a32d4,12,18,1024,23,24,8,26,28,30]: data+=value.to_bytes(4,'little')
 data+=fast()+command()+kle();h=0x811c9dc5
 for byte in data: h=((h^byte)*0x01000193)&0xffffffff
 return h
render='// Generated independently by phase9_5/reference/generate_contract_vectors.py.\n'+f'pub const PHASE95_CONTRACT_SIGNATURE: u32 = 0x{signature():08x};\n'+arr('KLR9_FAST_VECTOR',fast())+arr('KLR9_COMMAND_VECTOR',command())+arr('KLE9_VECTOR',kle())
if '--check' in sys.argv:
 if OUT.read_text(encoding='utf-8')!=render: raise SystemExit(f'stale vector file: {OUT}')
else: OUT.parent.mkdir(parents=True,exist_ok=True);OUT.write_text(render,encoding='utf-8',newline='\n')
