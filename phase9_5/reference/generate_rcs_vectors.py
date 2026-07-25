#!/usr/bin/env python3
"""Generate independent exact and float64 vectors for Phase 9.5 RCS physics."""
from __future__ import annotations
import argparse, json, math, struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
PACK=ROOT/'phase9_5'/'examples'/'firestorm-r9.kpe9'
OUT=ROOT/'phase9_5'/'generated'/'rcs_vectors_v1.rs'

def i32(b,o): return struct.unpack_from('<i',b,o)[0]
def half_away(n,d):
    sign=-1 if (n<0)^(d<0) else 1; n=abs(n); d=abs(d)
    q,r=divmod(n,d)
    if 2*r>=d:q+=1
    return sign*q
def mul(a,b,shift): return half_away(a*b,1<<shift)
def divs(a,b,shift): return half_away(a<<shift,b)
def interp(x,xs,ys):
    if x<=xs[0]: return ys[0]
    if x>=xs[-1]: return ys[-1]
    for i in range(1,len(xs)):
        if x<=xs[i]:
            fraction=max(0,min(65535,divs(x-xs[i-1],xs[i]-xs[i-1],16)))
            return ys[i-1]+mul(ys[i]-ys[i-1],fraction,16)
    raise AssertionError

def parse():
    b=PACK.read_bytes(); count=b[37]
    jets=[]
    for j in range(12):
        at=480+48*j
        jets.append({'p':[i32(b,at+4*k) for k in range(3)],'d':[i32(b,at+12+4*k) for k in range(3)],'thrust':i32(b,at+24),'isp':i32(b,at+28)})
    knots=[]
    for k in range(count):
        at=1056+16*k
        knots.append([i32(b,at+4*q) for q in range(4)])
    return jets,knots,i32(b,72)
def cross(p,f):
    return [mul(p[1],f[2],39)-mul(p[2],f[1],39),mul(p[2],f[0],39)-mul(p[0],f[2],39),mul(p[0],f[1],39)-mul(p[1],f[0],39)]
def case(indices,remaining,cg_q28,dt=1024):
    jets,knots,_=parse(); xs=[k[0] for k in knots]; thrusts=[k[2] for k in knots]; flows=[k[3] for k in knots]
    ts=interp(remaining,xs,thrusts); ms=interp(remaining,xs,flows)
    force=[0,0,0]; torque=[0,0,0]; mass_flow=0; impulse=0
    for j in indices:
        jet=jets[j]; thrust=mul(jet['thrust'],ts,30)
        f=[mul(thrust,v,30) for v in jet['d']]
        arm=[jet['p'][0]-cg_q28,jet['p'][1],jet['p'][2]]
        t=cross(arm,f)
        force=[a+b for a,b in zip(force,f)]; torque=[a+b for a,b in zip(torque,t)]
        denom=mul(jet['isp'],round(9.80665*(1<<16)),16)
        base_flow=divs(jet['thrust'],denom,21)
        mass_flow+=mul(base_flow,ms,30)
        impulse+=mul(thrust,dt,15)
    consumed=mul(mass_flow,dt,25)
    return {'force':force,'torque':torque,'mass_flow':mass_flow,'impulse':impulse,'consumed':consumed,'thrust_scale':ts,'mass_flow_scale':ms}
def farr(xs):return '['+', '.join(str(x) for x in xs)+']'
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--check',action='store_true');ap.add_argument('--report',action='store_true');a=ap.parse_args()
    jets,knots,wet=parse();cg=round(.95*(1<<28));balanced=case([4,5],wet,cg);single=case([0],wet,cg);half=case([4,5],wet//2,cg)
    text='// Generated independently by phase9_5/reference/generate_rcs_vectors.py. Do not edit.\n'
    for name,c in [('BALANCED',balanced),('SINGLE',single),('HALF_SUPPLY',half)]:
        text+=f'pub const {name}_FORCE_Q23: [i32; 3] = {farr(c["force"])};\n'
        text+=f'pub const {name}_TORQUE_Q12: [i32; 3] = {farr(c["torque"])};\n'
        text+=f'pub const {name}_MASS_FLOW_Q28: i32 = {c["mass_flow"]};\n'
        text+=f'pub const {name}_IMPULSE_Q26: i32 = {c["impulse"]};\n'
        text+=f'pub const {name}_CONSUMED_Q21: i32 = {c["consumed"]};\n'
        text+=f'pub const {name}_THRUST_SCALE_Q30: i32 = {c["thrust_scale"]};\n'
    text+='pub const FULL_PROPELLANT_Q21: i32 = '+str(wet)+';\n'
    if a.check:
        if not OUT.exists() or OUT.read_text()!=text: raise SystemExit('generated RCS vectors are stale')
    else: OUT.write_text(text)
    if a.report:
        thrust=1.0;isp=55.0;g0=9.80665;dt=1/256
        fixed_torque=balanced['torque'][1]/(1<<12); float_torque=2*(.55*thrust)
        fixed_flow=balanced['mass_flow']/(1<<28);float_flow=2*(thrust/(isp*g0))
        report={'balanced':balanced,'single':single,'half_supply':half,'float64':{'balanced_torque_nm':float_torque,'balanced_mass_flow_kg_s':float_flow},'relative_error':{'torque':abs(fixed_torque-float_torque)/float_torque,'mass_flow':abs(fixed_flow-float_flow)/float_flow}}
        print(json.dumps(report,sort_keys=True))
    return 0
if __name__=='__main__':raise SystemExit(main())
