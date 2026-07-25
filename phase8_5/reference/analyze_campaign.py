#!/usr/bin/env python3
"""Independent strict validator for the ordered Phase 8.5 KAS8 campaign stream."""
from __future__ import annotations
import argparse, hashlib, json, struct, zlib
from pathlib import Path
RUNS=64
RECORD=264
KAS=256
CONTRACT=0x08500001

def u16(data:bytes,at:int)->int:return struct.unpack_from('<H',data,at)[0]
def u32(data:bytes,at:int)->int:return struct.unpack_from('<I',data,at)[0]
def main()->int:
 p=argparse.ArgumentParser();p.add_argument('archive',type=Path);p.add_argument('--evidence',type=Path);a=p.parse_args();raw=a.archive.read_bytes()
 if len(raw)!=RUNS*RECORD:raise SystemExit(f'length {len(raw)} != {RUNS*RECORD}')
 variation=[];identities=[];alarms=0
 for index in range(RUNS):
  offset=index*RECORD
  if u32(raw,offset)!=index:raise SystemExit(f'run order failure at {index}')
  variation.append(u32(raw,offset+4));record=raw[offset+8:offset+RECORD]
  if record[:4]!=b'KAS8' or u16(record,4)!=8 or u16(record,6)!=32 or u16(record,8)!=KAS or u16(record,10)!=4:raise SystemExit(f'KAS8 framing failure at {index}')
  if u32(record,12)!=CONTRACT or not u32(record,16) or any(record[20:32]):raise SystemExit(f'KAS8 identity failure at {index}')
  if u32(record,KAS-4)!=(zlib.crc32(record[:KAS-4])&0xffffffff):raise SystemExit(f'KAS8 CRC failure at {index}')
  if record[34] or record[35] or record[106] or record[107] or any(record[156:KAS-4]):raise SystemExit(f'KAS8 reserved-byte failure at {index}')
  identities.append(u32(record,16));alarms+=u16(record,100)!=0
 result={'schema':'ksa64.phase8_5.independent-campaign-analysis-v1','runs':RUNS,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'stream_crc32':f'0x{zlib.crc32(b"".join(raw[i*RECORD+8:(i+1)*RECORD] for i in range(RUNS)))&0xffffffff:08x}','run_zero_nominal':variation[0]==0,'unique_record_identities':len(set(identities)),'alarmed_runs':alarms}
 if a.evidence:
  expected=json.loads(a.evidence.read_text())
  if expected['sha256']!=result['sha256'] or expected['records_crc32']!=result['stream_crc32'] or expected['runs']!=RUNS:raise SystemExit('evidence mismatch')
 print(json.dumps(result,indent=2));return 0
if __name__=='__main__':raise SystemExit(main())
