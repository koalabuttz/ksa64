#!/usr/bin/env python3
"""Independently reconstruct Phase 9.5 reference-pack identities and mass moments."""
from __future__ import annotations
import json, struct, sys, zlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
SRC=ROOT/'phase9_5'/'source-data'/'advanced-effectors-v1.json'
OUT=ROOT/'phase9_5'/'examples'
BASE=ROOT/'phase8'/'examples'/'firestorm54.kvp8'

def u32(b,o): return struct.unpack_from('<I',b,o)[0]
def i32(b,o): return struct.unpack_from('<i',b,o)[0]
def crc_ok(b): return u32(b,len(b)-4)==(zlib.crc32(b[:-4])&0xffffffff)
def number(field): return float(field['value'])
def fnv(data):
 h=0x811c9dc5
 for value in data: h=((h^value)*0x01000193)&0xffffffff
 return h

def main():
 source_bytes=SRC.read_bytes(); source=json.loads(source_bytes); base=BASE.read_bytes()
 report=json.loads((OUT/'compile-report.json').read_text())
 assert source['schema']=='ksa64.advanced-effector-source-v1' and len(report)==4
 base_mass=i32(base,36)/(1<<21); base_cg=i32(base,52)/(1<<28)
 provenance=fnv(source_bytes)
 expected={
  'Firestorm-C9': [(number(source['canard']['mass_each_kg']),number(source['canard']['station_from_nose_m']))]*4,
  'Firestorm-R9': [(number(source['firestorm_rcs']['tank_dry_mass_kg']),number(source['firestorm_rcs']['tank_station_m']))]+[(number(source['firestorm_rcs']['jet_hardware_mass_kg']), (number(source['firestorm_rcs']['fore_station_m'])+number(source['firestorm_rcs']['aft_station_m']))*.5 if i<4 else number(source['firestorm_rcs']['fore_station_m']) if i%2==0 else number(source['firestorm_rcs']['aft_station_m'])) for i in range(12)],
 }
 expected['Firestorm-M9']=expected['Firestorm-C9']+expected['Firestorm-R9']+[(.020,1.88)]
 expected['KSA-X1']=expected['Firestorm-C9']+[(number(source['research_rcs']['tank_dry_mass_kg']),number(source['research_rcs']['tank_station_m']))]+[(number(source['research_rcs']['jet_hardware_mass_kg']), (number(source['research_rcs']['fore_station_m'])+number(source['research_rcs']['aft_station_m']))*.5 if i<4 else number(source['research_rcs']['fore_station_m']) if i%2==0 else number(source['research_rcs']['aft_station_m'])) for i in range(12)]+[(.020,1.88)]
 for item in report:
  name=item['name']; stem=name.lower(); kvp=(OUT/f'{stem}.kvp8').read_bytes(); kpe=(OUT/f'{stem}.kpe9').read_bytes(); kpa=(OUT/f'{stem}.kpa9').read_bytes()
  assert len(kvp)==1024 and len(kpe)==2048 and len(kpa)==512
  assert kvp[:4]==b'KVP8' and kpe[:4]==b'KPE9' and kpa[:4]==b'KPA9'
  assert crc_ok(kvp) and crc_ok(kpe) and crc_ok(kpa)
  assert u32(kvp,16)==item['vehicle_identity']==u32(kpe,40)
  assert u32(kpe,16)==item['effector_identity']==u32(kpa,36)
  assert u32(kpa,16)==item['allocator_identity'] and u32(kvp,100)==provenance
  masses=expected[name]; total=base_mass+sum(m for m,_ in masses); moment=base_mass*base_cg+sum(m*x for m,x in masses); cg=moment/total
  assert abs(item['dry_mass_kg']-total)<1e-12 and abs(item['dry_cg_from_nose_m']-cg)<1e-12
  assert abs(i32(kvp,36)/(1<<21)-total)<=1/(1<<21) and abs(i32(kvp,52)/(1<<28)-cg)<=1/(1<<28)
  if item['canard_count']:
   assert all(i32(kpe,80+i*4)==round(.5*(1<<24)) for i in range(4))
  assert kpe[34]==item['canard_count'] and kpe[35]==item['jet_count']
 print(json.dumps({'variants':len(report),'provenance_identity':f'0x{provenance:08x}','status':'pass'},sort_keys=True))
if __name__=='__main__': main()
