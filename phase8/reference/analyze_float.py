#!/usr/bin/env python3
"""Independent float64 Phase 8 calm vertical/recovery analysis."""
from __future__ import annotations
import json, math, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
VEH=ROOT/'phase8/source-data/firestorm54-spatial.json'
MOTOR=ROOT/'phase8/source-data/aerotech-i211w-spatial.json'
MISSION=ROOT/'phase8/source-data/firestorm-i211-spatial-mission.json'
HOST=ROOT/'phase8/host-run-v1.json'
OUTPUT=ROOT/'phase8/float64-analysis-v1.json'
G0=9.80665; EARTH=6_371_000.0; R=287.05287; GAMMA=1.4
BASE_H=[0.,11000.,20000.,32000.,47000.,51000.,71000.,84852.]
LAPSE=[-.0065,0.,.001,.0028,0.,-.0028,-.002,0.]

def n(x):return float(x['value'])
def bases():
 t=[288.15];p=[101325.]
 for i in range(len(BASE_H)-1):
  dh=BASE_H[i+1]-BASE_H[i];tn=t[-1]+LAPSE[i]*dh
  pn=p[-1]*math.exp(-G0*dh/(R*t[-1])) if LAPSE[i]==0 else p[-1]*(t[-1]/tn)**(G0/(R*LAPSE[i]))
  t.append(tn);p.append(pn)
 return t,p
BT,BP=bases()
def atmosphere(z):
 h=EARTH*max(z,0)/(EARTH+max(z,0));i=len(BASE_H)-1
 for j in range(len(BASE_H)-1):
  if h<BASE_H[j+1]:i=j;break
 temp=BT[i]+LAPSE[i]*(h-BASE_H[i])
 press=BP[i]*math.exp(-G0*(h-BASE_H[i])/(R*BT[i])) if LAPSE[i]==0 else BP[i]*(BT[i]/temp)**(G0/(R*LAPSE[i]))
 return press/(R*temp),math.sqrt(GAMMA*R*temp),G0*(EARTH/(EARTH+max(z,0)))**2
def interp(points,x):
 if x<=points[0][0]:return points[0][1]
 for a,b in zip(points,points[1:]):
  if x<=b[0]:return a[1]+(b[1]-a[1])*(x-a[0])/(b[0]-a[0])
 return points[-1][1]
def main():
 vehicle=json.loads(VEH.read_text());motor=json.loads(MOTOR.read_text());mission=json.loads(MISSION.read_text());host=json.loads(HOST.read_text())
 dry=n(vehicle['declared_dry_mass_kg']);loaded=n(motor['loaded_mass_kg']);prop0=n(motor['propellant_mass_kg']);impulse=n(motor['total_impulse_ns']);burn=n(motor['burn_time_s'])
 area=math.pi*n(vehicle['diameter_m'])**2/4;curve=[(float(a),float(b)) for a,b in motor['thrust_curve']['knots']];cd=[(n(k['mach']),n(k['axial_cd'])) for k in vehicle['aero_seed']]
 rail=n(mission['rail_length_m'])-n(vehicle['rail_guides'][0]['from_tail_m']); main_alt=n(mission['main_deployment_altitude_m']);drogue_cda=n(vehicle['drogue_cda_m2']);main_cda=n(vehicle['main_cda_m2']);drogue_inflate=n(mission['drogue_inflation_time_s']);main_inflate=n(mission['main_inflation_time_s'])
 t=z=v=distance=0.;prop=prop0;phase='rail';deploy_t=0.;events={};max_z=max_v=max_a=max_q=0.
 while t<n(mission['max_mission_time_s']):
  dt=.01 if phase in ('rail','powered') else .02 if phase=='coast' else .05
  rho,sound,g=atmosphere(z);speed=abs(v);q=.5*rho*speed*speed;thrust=interp(curve,t) if phase in ('rail','powered') and t<burn else 0.
  if thrust>0:prop=max(0.,prop-prop0*(thrust*dt/impulse))
  if t>=burn:prop=0.
  mass=dry+(loaded-prop0)+prop
  if phase in ('rail','powered','coast'):
   drag=q*area*interp(cd,speed/sound)*(1 if v>=0 else -1);a=(thrust-drag)/mass-g
   if phase=='rail':a=max(0.,a)
  else:
   target=drogue_cda if phase=='drogue' else main_cda;inflate=drogue_inflate if phase=='drogue' else main_inflate;cda=target*min(1.,max(0.,(t-deploy_t)/inflate));drag=q*cda*(1 if v>=0 else -1);a=-drag/mass-g
  v_prev=v;v+=a*dt;z+=v*dt;t+=dt
  max_z=max(max_z,z);max_v=max(max_v,abs(v));max_a=max(max_a,abs(a));max_q=max(max_q,q)
  if phase=='rail':
   distance=z
   if distance>=rail:phase='powered';events['rail_exit_time_s']=t
  if phase=='powered' and t>=burn:phase='coast';events['burnout_time_s']=t
  if phase=='coast' and v_prev>=0 and v<0:phase='drogue';deploy_t=t;events['apogee_time_s']=t;events['drogue_time_s']=t
  if phase=='drogue' and v<0 and z<=main_alt:phase='main';deploy_t=t;events['main_time_s']=t
  if phase in ('drogue','main') and z<=0 and v<0:z=0.;events['landing_time_s']=t;break
 analysis={'schema':'ksa64.phase8-float64-analysis-v1','method':'independent float64 US76 vertical ascent and point-mass recovery','metrics':{**events,'apogee_m':max_z,'max_speed_mps':max_v,'max_acceleration_mps2':max_a,'max_dynamic_pressure_pa':max_q,'landing_position_m':[0.,0.,0.]}}
 deltas={k:analysis['metrics'][k]-host[k] for k in ('rail_exit_time_s','burnout_time_s','apogee_time_s','drogue_time_s','main_time_s','landing_time_s','apogee_m','max_speed_mps','max_acceleration_mps2','max_dynamic_pressure_pa')};analysis['host_deltas']=deltas
 limits={'event_time_s':.051,'apogee_fraction':.005,'landing_position_m':5.0,'attitude_deg':.5};analysis['limits']=limits
 if abs(deltas['apogee_m'])/host['apogee_m']>limits['apogee_fraction']:raise SystemExit(f"apogee delta exceeds gate: {deltas['apogee_m']}")
 for key in ('rail_exit_time_s','burnout_time_s','apogee_time_s','drogue_time_s','main_time_s','landing_time_s'):
  if abs(deltas[key])>limits['event_time_s']:raise SystemExit(f"{key} delta exceeds gate: {deltas[key]}")
 initial=host['trace'][0]['quaternion'];dot_min=min(abs(sum(a*b for a,b in zip(initial,p['quaternion']))) for p in host['trace'] if p['phase']<=2);attitude=2*math.degrees(math.acos(min(1.,dot_min)));analysis['host_predeployment_attitude_excursion_deg']=attitude
 if attitude>limits['attitude_deg']:raise SystemExit(f"attitude excursion exceeds gate: {attitude}")
 rendered=json.dumps(analysis,indent=2,sort_keys=True)+'\n'
 if '--check' in sys.argv:
  if OUTPUT.read_text()!=rendered:raise SystemExit(f'stale generated file: {OUTPUT}')
 else:OUTPUT.write_text(rendered,newline='\n')
if __name__=='__main__':main()
