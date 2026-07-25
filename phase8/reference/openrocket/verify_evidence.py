#!/usr/bin/env python3
"""Verify checked Phase 8 external evidence without installing OpenRocket."""
from __future__ import annotations
import hashlib,json,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 manifest=json.loads((ROOT/'phase8/openrocket/openrocket-settings-v1.json').read_text(encoding='utf-8'))
 for name,expected in manifest['artifacts'].items():
  path=ROOT/name
  if not path.is_file():raise SystemExit(f'missing OpenRocket evidence: {name}')
  actual=sha(path)
  if actual!=expected:raise SystemExit(f'OpenRocket evidence hash mismatch: {name}: {actual}')
 summary=json.loads((ROOT/'phase8/openrocket/openrocket-summary-v1.json').read_text(encoding='utf-8'))
 if summary['tool']!='OpenRocket 24.12' or len(summary['cases'])!=2:raise SystemExit('invalid OpenRocket summary identity')
 design=(ROOT/'phase8/openrocket/firestorm54-i211w-v1.ork').read_text(encoding='utf-8')
 if 'creator="OpenRocket 24.12"' not in design or 'I211W' not in design:raise SystemExit('invalid OpenRocket design')
 for name in ('openrocket-calm-v1.csv','openrocket-crosswind-5mps-v1.csv'):
  text=(ROOT/'phase8/openrocket'/name).read_text(encoding='utf-8-sig')
  if 'FlightDataType.TYPE_TIME' not in text or 'Event GROUND_HIT' not in text:raise SystemExit(f'invalid OpenRocket CSV: {name}')
 comparison=json.loads((ROOT/'phase8/openrocket/comparison-v1.json').read_text(encoding='utf-8'))
 if not comparison['all_passed']:raise SystemExit('OpenRocket comparison gate failed')
 print(f"verified {len(manifest['artifacts'])} hashed OpenRocket artifacts and {len(comparison['checks'])} acceptance checks")
if __name__=='__main__':main()
