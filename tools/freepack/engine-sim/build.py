import sys, os, json, numpy as np, wave
sys.path.insert(0,'/tmp/claude-0/es/work'); os.chdir('/tmp/claude-0/es/work')
os.environ['GAIN']='1'
from gen import render
SR=44100
S=[]; T=[0.0]; cuts={}
def seg(t,thr,d0,d1=None,gear=-1):
    S.append((t,thr,d0,d0 if d1 is None else d1,gear)); T[0]+=t
def mark(name,length,pre=0.02): cuts[name]=(T[0]-pre,length)
seg(3,0,0); seg(1,0,0)
mark('Idle01',3.0,0); mark('IdleLoop',4.0,0); seg(4.2,0,0)
seg(0.2,0,0); mark('Rev01',3.0); seg(0.42,1,0); seg(3.0,0,0); seg(1.0,0,0)
def pull(name,length,a,b,lead=0.35):
    seg(0.5,0,a); seg(lead,0,a); mark(name,length); seg(length+0.1,1,a,b)
pull('LowTorque01',1.5,2200,5600); pull('LowTorque02',1.8,2000,6000)
pull('MidTorque01',1.8,3600,7600); pull('MidTorque02',1.8,3900,7900)
pull('HighTorque01',1.8,5600,8800); pull('HighTorque02',1.8,5900,9000)
pull('PowerBand01',1.8,7700,9300); pull('PowerBand02',1.8,8100,9400); pull('PowerBand03',1.8,7900,9200)
def sputter(name,length,top,low,cut):
    seg(0.5,0,top); seg(0.3,1,top); mark(name,length,0.0)
    seg(cut,0,top,low); seg(length-cut+0.1,1,low,low+(top-low)*0.45)
sputter('Sputter01',1.35,9200,6900,0.14); sputter('Sputter02',1.5,9300,6600,0.18); sputter('Sputter03',1.1,9100,7200,0.11)
def decel(name,length,a,b):
    seg(0.4,1,a); mark(name,length,0.0); seg(length+0.1,0,a,b)
decel('Decel01',2.0,9000,2600); decel('Decel02',1.8,8600,2400)
seg(0.6,1,4442); mark('OnLoop',4.0,0); seg(4.2,1,4442)
seg(0.8,0,3823); mark('OffLoop',4.0,0); seg(4.2,0,3823)
seg(1.5,0,0)
json.dump(cuts,open('/tmp/claude-0/es/set/cuts.json','w'))
log=render('set600',S,IDLEPOS=0.997,IR='real_engine_1',OUTLET=800,PIPE=30)
open("/tmp/claude-0/es/set/log.txt","w").write(log); print('total',T[0])
