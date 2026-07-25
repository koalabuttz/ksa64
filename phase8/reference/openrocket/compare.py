#!/usr/bin/env python3
"""Compare KSA64 exact evidence with the aligned OpenRocket 24.12 exports."""
from __future__ import annotations
import csv,json,math,re,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
OUT=ROOT/'phase8/openrocket/comparison-v1.json'; REPORT=ROOT/'phase8/openrocket/COMPARISON.md'

def load(path):return json.loads((ROOT/path).read_text(encoding='utf-8'))
def rel(a,b):return abs(a-b)/abs(b) if b else abs(a-b)
def event_speed(run,mask):
 p=next(p for p in run['trace'] if p['events']&mask)
 return math.sqrt(sum(v*v for v in p['velocity_mps']))
def parse_csv(name):
 path=ROOT/'phase8/openrocket'/name;events={};rows=[]
 for line in path.read_text(encoding='utf-8-sig').splitlines():
  m=re.match(r'# Event ([A-Z_]+) occurred at t=([0-9.]+) seconds',line)
  if m:events.setdefault(m.group(1),float(m.group(2)))
  elif line and not line.startswith('#'):
   rows.append([float(x) if x!='NaN' else math.nan for x in line.split(',')])
 return events,rows

def metric(name,k,o,limit,kind='relative'):
 error=abs(k-o) if kind=='absolute' else rel(k,o)
 return {'metric':name,'ksa64':k,'openrocket':o,'error':error,'limit':limit,'kind':kind,'passed':error<=limit}
def main():
 calm=load('phase8/host-run-v1.json');wind=load('phase8/host-run-crosswind-5mps-v1.json');ors=load('phase8/openrocket/openrocket-summary-v1.json');comp=load('phase8/examples/compile-report.json');vehicle=load('phase8/source-data/firestorm54-spatial.json');motor=load('phase8/source-data/aerotech-i211w-spatial.json')
 orc={c['name']:c for c in ors['cases']};calm_or=orc['calm'];wind_or=orc['steady-crosswind-5mps']
 ce,cr=parse_csv('openrocket-calm-v1.csv');we,wr=parse_csv('openrocket-crosswind-5mps-v1.csv')
 checks=[]
 for label,k,o,events in [('calm',calm,calm_or,ce),('crosswind5',wind,wind_or,we)]:
  checks += [
   metric(label+'.rail_exit_velocity_mps',event_speed(k,1),o['rail_exit_velocity_mps'],.05),
   metric(label+'.max_velocity_mps',k['max_speed_mps'],o['max_velocity_mps'],.05),
   metric(label+'.apogee_m',k['apogee_m'],o['apogee_m'],.05),
   metric(label+'.burnout_time_s',k['burnout_time_s'],events['BURNOUT'],.05),
   metric(label+'.max_dynamic_pressure_pa',k['max_dynamic_pressure_pa'],o['max_dynamic_pressure_pa'],.10),
   metric(label+'.time_to_apogee_s',k['apogee_time_s'],o['time_to_apogee_s'],.5,'absolute'),
   metric(label+'.max_aoa_deg',k['max_angle_of_attack_deg'],math.degrees(o['max_aoa_rad']),2.0,'absolute'),
  ]
 landing_limit=max(50.0,.15*wind_or['landing_distance_m'])
 checks.append(metric('crosswind5.landing_distance_m',abs(wind['landing_position_m'][0]),wind_or['landing_distance_m'],landing_limit,'absolute'))
 # Installed dry state: dry vehicle plus dry motor casing at its declared station.
 dry_mass=float(vehicle['declared_dry_mass_kg']['value']); dry_cg=comp['dry_cg_from_nose_m']; motor_dry=float(motor['loaded_mass_kg']['value'])-float(motor['propellant_mass_kg']['value']); motor_cg=float(vehicle['length_m']['value'])-float(motor['dry_cg_from_aft_m']['value']); installed_cg=(dry_mass*dry_cg+motor_dry*motor_cg)/(dry_mass+motor_dry); ksa_cp=comp['derived_cp_from_nose_m']; ksa_margin=(ksa_cp-installed_cg)/float(vehicle['diameter_m']['value'])
 burnout=min(cr,key=lambda r:abs(r[0]-ce['BURNOUT'])); or_cp,or_cg,or_margin=burnout[12:15]
 caliber=float(vehicle['diameter_m']['value'])
 checks += [
  metric('geometry.dry_mass_kg',dry_mass,ors['aligned_dry_mass_kg'],.005),
  metric('geometry.cp_caliber_delta',ksa_cp,or_cp,.25*caliber,'absolute'),
  metric('geometry.cg_caliber_delta',installed_cg,or_cg,.25*caliber,'absolute'),
  metric('geometry.static_margin_calibers',ksa_margin,or_margin,.25,'absolute'),
 ]
 # OpenRocket direction=0 means wind from +Y. Thus KSA +east maps to OR -Y.
 or_apogee=min(wr,key=lambda r:abs(r[0]-wind_or['time_to_apogee_s']))
 directions={
  'weathercocking':{'ksa64_east_m':next(p for p in reversed(wind['trace']) if p['phase']<=2)['position_m'][0],'openrocket_mapped_east_m':-or_apogee[3]},
  'landing_drift':{'ksa64_east_m':wind['landing_position_m'][0],'openrocket_mapped_east_m':-wr[-1][3]},
 }
 direction_pass=all(a['ksa64_east_m']*a['openrocket_mapped_east_m']>0 for a in directions.values())
 result={'schema':'ksa64.openrocket-comparison-v1','tool':'OpenRocket 24.12','checks':checks,'directions':directions,'direction_passed':direction_pass,'all_passed':all(c['passed'] for c in checks) and direction_pass,'interpretation':{'drag_correction':'KSA64 Cd knots were replaced with rounded effective coefficients read from the aligned OpenRocket export before rerunning KSA64. This is a documented model-source correction, not a fit against trajectory outcomes.','low_q_scope':'Directional small-angle aerodynamics are active only after rail clearance and at q >= 50 Pa. Both tools show AoA becoming singular near apogee as airspeed approaches zero.','authority':'Engineering comparison only; not launch approval, certification, regulatory, or safety authority.'}}
 rendered=json.dumps(result,indent=2,sort_keys=True)+'\n'
 md=['# Phase 8 OpenRocket comparison','','OpenRocket 24.12 was run headlessly with the checked Java harness. The `.ork`, native CSV exports, settings, and hashes are retained beside this report.','','| Check | KSA64 | OpenRocket | Error | Limit | Result |','|---|---:|---:|---:|---:|:---:|']
 for c in checks:
  suffix='%' if c['kind']=='relative' else ''
  e=100*c['error'] if suffix else c['error']; lim=100*c['limit'] if suffix else c['limit']
  md.append(f"| {c['metric']} | {c['ksa64']:.6g} | {c['openrocket']:.6g} | {e:.4g}{suffix} | {lim:.4g}{suffix} | {'PASS' if c['passed'] else 'FAIL'} |")
 md += ['','Weathercocking and recovery drift agree after the documented OpenRocket-to-ENU axis mapping.' if direction_pass else 'Direction comparison failed.','','The original Phase 7 axial-Cd placeholder was replaced by rounded effective coefficients from the aligned OpenRocket geometry before KSA64 was rerun. The near-apogee AoA singularity is explicitly outside the directional model below 50 Pa; state propagation continues.','','This is engineering evidence, not launch approval, certification, regulatory guidance, or safety authority.','']
 if '--check' in sys.argv:
  if OUT.read_text(encoding='utf-8')!=rendered or REPORT.read_text(encoding='utf-8')!='\n'.join(md):raise SystemExit('stale OpenRocket comparison evidence')
 else:
  OUT.write_text(rendered,encoding='utf-8',newline='\n');REPORT.write_text('\n'.join(md),encoding='utf-8',newline='\n')
 if not result['all_passed']:
  for c in checks:
   if not c['passed']:print('FAIL',c,file=sys.stderr)
  raise SystemExit('OpenRocket acceptance gate failed')
if __name__=='__main__':main()
