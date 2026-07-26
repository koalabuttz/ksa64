#!/usr/bin/env python3
"""Independent float64 nominal Phase 9.5 mission evidence.

This intentionally reimplements atmosphere, thrust/mass depletion, rail/ascent,
8 Hz measured-state recovery sequencing, inflation, descent, and landing.  It
uses the reviewed source data plus the compiler's provenance-bearing mass
inventory; it does not call the Rust evaluator or decode KSA64 state.
"""
from __future__ import annotations
import json,math,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
VEH=ROOT/'phase8/source-data/firestorm54-spatial.json';MOTOR=ROOT/'phase8/source-data/aerotech-i211w-spatial.json';MISSION=ROOT/'phase8/source-data/firestorm-i211-spatial-mission.json'
COMPILE=ROOT/'phase9_5/examples/compile-report.json';HOST=ROOT/'phase9_5/evidence/integrated/integrated-cases-v1.json';OUT=ROOT/'phase9_5/evidence/integrated/float64-integrated-v1.json'
G0=9.80665;EARTH=6_371_000.;R=287.05287;GAMMA=1.4;BASE_H=[0.,11000.,20000.,32000.,47000.,51000.,71000.,84852.];LAPSE=[-.0065,0.,.001,.0028,0.,-.0028,-.002,0.]
def n(x):return float(x['value'])
def bases():
 t=[288.15];p=[101325.]
 for i in range(len(BASE_H)-1):
  dh=BASE_H[i+1]-BASE_H[i];tn=t[-1]+LAPSE[i]*dh;pn=p[-1]*math.exp(-G0*dh/(R*t[-1])) if LAPSE[i]==0 else p[-1]*(t[-1]/tn)**(G0/(R*LAPSE[i]));t.append(tn);p.append(pn)
 return t,p
BT,BP=bases()
def atmosphere(z):
 h=EARTH*max(z,0)/(EARTH+max(z,0));i=len(BASE_H)-1
 for j in range(len(BASE_H)-1):
  if h<BASE_H[j+1]:i=j;break
 temp=BT[i]+LAPSE[i]*(h-BASE_H[i]);press=BP[i]*math.exp(-G0*(h-BASE_H[i])/(R*BT[i])) if LAPSE[i]==0 else BP[i]*(BT[i]/temp)**(G0/(R*LAPSE[i]));return press/(R*temp),math.sqrt(GAMMA*R*temp),G0*(EARTH/(EARTH+max(z,0)))**2
def interp(points,x):
 if x<=points[0][0]:return points[0][1]
 for a,b in zip(points,points[1:]):
  if x<=b[0]:return a[1]+(b[1]-a[1])*(x-a[0])/(b[0]-a[0])
 return points[-1][1]
def simulate(dry,rcs_prop,vehicle,motor,mission):
 loaded=n(motor['loaded_mass_kg']);prop0=n(motor['propellant_mass_kg']);impulse=n(motor['total_impulse_ns']);burn=n(motor['burn_time_s']);area=math.pi*n(vehicle['diameter_m'])**2/4;curve=[(float(a),float(b))for a,b in motor['thrust_curve']['knots']];cd=[(n(k['mach']),n(k['axial_cd']))for k in vehicle['aero_seed']];rail=n(mission['rail_length_m'])-n(vehicle['rail_guides'][0]['from_tail_m']);main_alt=n(mission['main_deployment_altitude_m']);drogue_cda=n(vehicle['drogue_cda_m2']);main_cda=n(vehicle['main_cda_m2']);dinf=n(mission['drogue_inflation_time_s']);minf=n(mission['main_inflation_time_s'])
 dt=.001;t=z=v=distance=0.;prop=prop0;phase='rail';deploy_t=0.;events={};max_z=max_v=max_a=max_q=0.;next_nav=0.;descending=0
 while t<n(mission['max_mission_time_s']):
  rho,sound,g=atmosphere(z);speed=abs(v);q=.5*rho*speed*speed;thrust=interp(curve,t)if phase in('rail','powered')and t<burn else 0.
  if thrust>0:prop=max(0.,prop-prop0*(thrust*dt/impulse))
  if t>=burn:prop=0.
  mass=dry+(loaded-prop0)+prop+rcs_prop
  if phase in('rail','powered','coast'):
   drag=q*area*interp(cd,speed/sound)*(1 if v>=0 else -1);a=(thrust-drag)/mass-g
   if phase=='rail':a=max(0.,a)
  else:
   target=drogue_cda if phase=='drogue' else main_cda;inflate=dinf if phase=='drogue' else minf;cda=target*min(1.,max(0.,(t-deploy_t)/inflate));drag=q*cda*(1 if v>=0 else -1);a=-drag/mass-g
  old_v=v;v+=a*dt;z+=v*dt;t+=dt;max_z=max(max_z,z);max_v=max(max_v,abs(v));max_a=max(max_a,abs(a));max_q=max(max_q,q)
  if phase=='rail':
   distance=z
   if distance>=rail:phase='powered';events['rail_exit_time_s']=t
  if phase=='powered'and t>=burn:phase='coast';events['burnout_time_s']=t
  if phase=='coast'and old_v>=0>v:events['apogee_time_s']=t
  while t+1e-12>=next_nav:
   if phase=='coast':
    descending=descending+1 if v<0 else 0
    if descending>=2:phase='drogue';deploy_t=t;events['drogue_time_s']=t
   elif phase=='drogue'and v<0 and z<=main_alt:phase='main';deploy_t=t;events['main_time_s']=t
   next_nav+=.125
  if phase in('drogue','main')and z<=0 and v<0:z=0.;events['landing_time_s']=t;break
 return {**events,'apogee_m':max_z,'max_speed_mps':max_v,'max_acceleration_mps2':max_a,'max_dynamic_pressure_pa':max_q}
def main():
 vehicle=json.loads(VEH.read_text());motor=json.loads(MOTOR.read_text());mission=json.loads(MISSION.read_text());inventory={r['name'].lower():r for r in json.loads(COMPILE.read_text())};host={r['name']:r for r in json.loads(HOST.read_text())};cases=[];limits={'event_time_s':.30,'apogee_fraction':.005,'max_speed_fraction':.015,'max_q_fraction':.03}
 for case,key,rcs in [('firestorm-c9-nominal','firestorm-c9',0.),('firestorm-r9-nominal','firestorm-r9',.1),('firestorm-m9-nominal','firestorm-m9',.1)]:
  f=simulate(inventory[key]['dry_mass_kg'],rcs,vehicle,motor,mission);h=host[case];exact={'rail_exit_time_s':h['rail_exit_time_q18']/2**18,'burnout_time_s':h['burnout_time_q18']/2**18,'apogee_time_s':h['apogee_time_q18']/2**18,'drogue_time_s':h['drogue_time_q18']/2**18,'main_time_s':h['main_time_q18']/2**18,'landing_time_s':h['landing_time_q18']/2**18,'apogee_m':h['apogee_q13']/2**13,'max_speed_mps':h['max_speed_q19']/2**19,'max_dynamic_pressure_pa':h['max_dynamic_pressure_q13']/2**13};delta={k:f[k]-exact[k]for k in exact};passed=all(abs(delta[k])<=limits['event_time_s']for k in ('rail_exit_time_s','burnout_time_s','apogee_time_s','drogue_time_s','main_time_s','landing_time_s'))and abs(delta['apogee_m'])/exact['apogee_m']<=limits['apogee_fraction']and abs(delta['max_speed_mps'])/exact['max_speed_mps']<=limits['max_speed_fraction']and abs(delta['max_dynamic_pressure_pa'])/exact['max_dynamic_pressure_pa']<=limits['max_q_fraction'];cases.append({'case':case,'float64':f,'exact':exact,'delta':delta,'passed':passed})
 result={'schema':'ksa64.phase9_5-integrated-float64-v1','method':'independent float64 US76 vertical dynamics with 8 Hz measured-state recovery sequencing','parameter_policy':'source data plus provenance-bearing compiled derivative mass inventory','limits':limits,'cases':cases,'all_passed':all(c['passed']for c in cases),'scope':'Complete nominal ascent, coast, recovery, and landing for each accepted derivative. Canard and RCS force/torque/depletion are covered separately by independent analytic vector suites.'};render=json.dumps(result,indent=2,sort_keys=True)+'\n'
 if '--check'in sys.argv:
  if OUT.read_text()!=render:raise SystemExit('stale float64 evidence')
 else:OUT.write_text(render,newline='\n')
 if not result['all_passed']:
  for c in cases:
   if not c['passed']:print(c['case'],c['delta'])
  raise SystemExit('integrated float64 gate failed')
 print('validated',len(cases),'complete nominal missions')
if __name__=='__main__':main()
