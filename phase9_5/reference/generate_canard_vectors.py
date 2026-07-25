#!/usr/bin/env python3
"""Independent exact and float64 canard fixtures for Phase 9.5."""
from __future__ import annotations
import json, math, struct, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
PACK=ROOT/'phase9_5'/'examples'/'firestorm-c9.kpe9'
OUT=ROOT/'phase9_5'/'generated'/'canard_vectors_v1.rs'
MAX_AOA=70276238; MAX_MACH=13421773; TWO_PI=1686629714

def i16(b,o): return struct.unpack_from('<h',b,o)[0]
def i32(b,o): return struct.unpack_from('<i',b,o)[0]
def mul(a,b,s):
 neg=(a<0)^(b<0); value=(abs(a)*abs(b)+(1<<(s-1) if s else 0))>>s
 return -value if neg else value
def div(n,d,s):
 neg=(n<0)^(d<0); value=((abs(n)<<s)+abs(d)//2)//abs(d)
 return -value if neg else value
def interp(x,xs,ys):
 if x<=xs[0]: return ys[0]
 if x>=xs[-1]: return ys[-1]
 for i in range(len(xs)-1):
  if x<xs[i+1]: return ys[i]+mul(ys[i+1]-ys[i],div(x-xs[i],xs[i+1]-xs[i],16),16)
 raise AssertionError
def cross(a,b): return [mul(a[1],b[2],29)-mul(a[2],b[1],29),mul(a[2],b[0],29)-mul(a[0],b[2],29),mul(a[0],b[1],29)-mul(a[1],b[0],29)]
def evaluate(turns,mach=1<<23,q=5000<<13,cg=250000000,limits=None):
 b=PACK.read_bytes(); count=b[36]
 xs=[i32(b,352+i*16) for i in range(count)]; control=interp(mach,xs,[i32(b,356+i*16) for i in range(count)]); dragc=interp(mach,xs,[i32(b,360+i*16) for i in range(count)]); hingec=interp(mach,xs,[i32(b,364+i*16) for i in range(count)])
 limits=limits or [i32(b,80+i*4) for i in range(4)]; totalf=[0,0,0]; totalt=[0,0,0]; hinges=[]; effective=[]; mask=0
 for index,turn in enumerate(turns):
  at=96+index*64; pos=[i32(b,at+i*4) for i in range(3)]; normal=[i16(b,at+12+i*2) for i in range(3)]; root=i32(b,at+24); tip=i32(b,at+28); span=i32(b,at+32); limit=i16(b,at+56); turn=max(-limit,min(limit,turn)); angle=mul(turn,TWO_PI,16)
  area=mul(root+tip,span,28)//2; chord=(root+tip)//2; qa=mul(q,area,28); qac=mul(qa,chord,17); hp=mul(qac,hingec,24); requested=abs(mul(hp,angle,28)); limited=requested>limits[index]
  if limited:
   ratio=max(0,min(1<<30,div(limits[index],requested,30))); et=mul(turn,ratio,30); ea=mul(angle,ratio,30); mask|=1<<index
  else: et=turn;ea=angle
  normal_force=mul(mul(qa,control,24),ea,28); a2=mul(ea,ea,28); induced=max(0,mul(mul(qa,dragc,24),a2,28)); force=[-induced,mul(normal_force,normal[1],15),mul(normal_force,normal[2],15)]; torque=cross([pos[0]-cg,pos[1],pos[2]],force); hinge=abs(mul(hp,ea,28))
  totalf=[a+c for a,c in zip(totalf,force)];totalt=[a+c for a,c in zip(totalt,torque)];hinges.append(hinge);effective.append(et)
 return dict(force=totalf,torque=totalt,drag=-totalf[0],hinge=hinges,effective=effective,mask=mask)
def render():
 cases=[('PITCH',[1820,-1820,0,0],None),('ROLL',[1820,1820,0,0],None),('LOAD',[1820]*4,[1<<12]*4)]
 lines=['// Generated independently by phase9_5/reference/generate_canard_vectors.py.']
 for name,turns,limits in cases:
  value=evaluate(turns,q=(20000 if name=='LOAD' else 5000)<<13,limits=limits)
  lines += [f'pub const {name}_TURN16: [i16; 4] = {turns!r};'.replace("'",''),f'pub const {name}_FORCE_Q13: [i32; 3] = {value["force"]!r};'.replace("'",''),f'pub const {name}_TORQUE_Q12: [i32; 3] = {value["torque"]!r};'.replace("'",''),f'pub const {name}_HINGE_Q24: [i32; 4] = {value["hinge"]!r};'.replace("'",''),f'pub const {name}_EFFECTIVE_TURN16: [i16; 4] = {value["effective"]!r};'.replace("'",''),f'pub const {name}_MASK: u8 = {value["mask"]};']
 return '\n'.join(lines)+'\n'
def float_report():
 b=PACK.read_bytes(); report={}
 for name,turns,q in [('pitch',[1820,-1820,0,0],5000),('roll',[1820,1820,0,0],5000)]:
  exact=evaluate(turns,q=q<<13); fixed_force=[v/(1<<13) for v in exact['force']]; fixed_torque=[v/(1<<12) for v in exact['torque']]
  # Re-evaluate the same bounded model in ordinary SI float64.
  mach=.5; knots=[(i32(b,352+i*16)/(1<<24),i32(b,356+i*16)/(1<<24),i32(b,360+i*16)/(1<<24)) for i in range(b[36])]
  lo=next(i for i in range(len(knots)-1) if mach<=knots[i+1][0]); f=(mach-knots[lo][0])/(knots[lo+1][0]-knots[lo][0]); control=knots[lo][1]+f*(knots[lo+1][1]-knots[lo][1]); drag=knots[lo][2]+f*(knots[lo+1][2]-knots[lo][2]); ff=[0.,0.,0.];tt=[0.,0.,0.]
  for i,turn in enumerate(turns):
   at=96+i*64; pos=[i32(b,at+j*4)/(1<<28) for j in range(3)]; normal=[i16(b,at+12+j*2)/(1<<15) for j in range(3)]; root=i32(b,at+24)/(1<<28);tip=i32(b,at+28)/(1<<28);span=i32(b,at+32)/(1<<28); angle=turn/65536*2*math.pi;area=(root+tip)*span/2;nf=q*area*control*angle;ind=q*area*drag*angle*angle;force=[-ind,nf*normal[1],nf*normal[2]];arm=[pos[0]-250000000/(1<<28),pos[1],pos[2]];torque=[arm[1]*force[2]-arm[2]*force[1],arm[2]*force[0]-arm[0]*force[2],arm[0]*force[1]-arm[1]*force[0]];ff=[a+c for a,c in zip(ff,force)];tt=[a+c for a,c in zip(tt,torque)]
  errors=[abs(a-b)/max(abs(b),1e-9) for a,b in zip(fixed_force+fixed_torque,ff+tt) if abs(b)>1e-8];report[name]={'max_relative_error':max(errors,default=0.0),'fixed_force_n':fixed_force,'float_force_n':ff,'fixed_torque_nm':fixed_torque,'float_torque_nm':tt}
 return report
rendered=render()
if '--check' in sys.argv:
 if OUT.read_text()!=rendered: raise SystemExit('stale canard vectors')
else: OUT.parent.mkdir(parents=True,exist_ok=True);OUT.write_text(rendered,newline='\n')
if '--report' in sys.argv: print(json.dumps(float_report(),sort_keys=True))
