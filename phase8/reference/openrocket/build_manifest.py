#!/usr/bin/env python3
"""Build the checked OpenRocket 24.12 evidence manifest without downloading tools."""
from __future__ import annotations
import argparse, hashlib, json, subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
OUTPUT=ROOT/'phase8/openrocket/openrocket-settings-v1.json'
RAW=[
 'phase8/openrocket/firestorm54-i211w-v1.ork',
 'phase8/openrocket/openrocket-calm-v1.csv',
 'phase8/openrocket/openrocket-crosswind-5mps-v1.csv',
 'phase8/openrocket/openrocket-summary-v1.json',
 'phase8/reference/openrocket/FirestormEvidence.java',
 'phase8/source-data/firestorm54-spatial.json',
 'phase8/source-data/aerotech-i211w-spatial.json',
 'phase8/source-data/firestorm-i211-spatial-mission.json',
 'phase8/examples/firestorm54.kvp8',
 'phase8/examples/aerotech-i211w.kmp8',
]

def sha(path:Path)->str:return hashlib.sha256(path.read_bytes()).hexdigest()
def main()->None:
 ap=argparse.ArgumentParser();ap.add_argument('--jar',type=Path);ap.add_argument('--java',type=Path);ap.add_argument('--check',action='store_true');a=ap.parse_args()
 jar_hash='4959b72f52f5f607941e9722abbb7b7f0c4a38ebbbf84204a329db9f31c4f897'
 if a.jar:
  actual=sha(a.jar)
  if actual!=jar_hash:raise SystemExit(f'wrong OpenRocket JAR hash: {actual}')
 java='Microsoft OpenJDK 17.0.20+8-LTS used for accepted run'
 if a.check and not a.java and OUTPUT.exists():java=json.loads(OUTPUT.read_text(encoding='utf-8'))['tool']['java_runtime']
 if a.java:
  result=subprocess.run([str(a.java),'-version'],capture_output=True,text=True,check=True)
  java=(result.stderr or result.stdout).strip().replace('\n','; ')
 manifest={
  'schema':'ksa64.openrocket-settings-v1',
  'tool':{'name':'OpenRocket','version':'24.12','release':'https://github.com/openrocket/openrocket/releases/tag/release-24.12','jar_sha256':jar_hash,'java_runtime':java},
  'model':{
   'vehicle':'Giant Leap Firestorm 54 / AeroTech I211W',
   'physical_rail_length_m':2.0,
   'aft_guide_from_tail_m':0.2794,
   'openrocket_effective_launch_rod_m':1.7206,
   'launch_rod_alignment_note':'OpenRocket launch-rod length is displacement to its LAUNCHROD event; KSA64 clears the aft guide from a physical 2 m rail, so the aligned OpenRocket displacement is 2.0 - 0.2794 m.',
   'launch_altitude_m':0.0,'isa_atmosphere':True,'integration_step_s':0.01,'random_seed':0x4b534138,
   'calm_wind_mps':0.0,'crosswind_mps':5.0,'wind_deviation_mps':0.0,'turbulence_intensity':0.0,
   'coordinate_mapping':'KSA64 +east = OpenRocket -Y for the accepted direction=0 steady-wind case.',
   'directional_aero_min_dynamic_pressure_pa':50.0,
   'directional_aero_note':'AoA acceptance begins after rail clearance and ends when q falls below 50 Pa; below that threshold the air-relative direction is singular and directional forces are mission-insignificant.'
  },
  'artifacts':{p:sha(ROOT/p) for p in RAW},
 }
 rendered=json.dumps(manifest,indent=2,sort_keys=True)+'\n'
 if a.check:
  if OUTPUT.read_text(encoding='utf-8')!=rendered:raise SystemExit(f'stale manifest: {OUTPUT}')
 else:OUTPUT.write_text(rendered,encoding='utf-8',newline='\n')
if __name__=='__main__':main()
