import sys, json, numpy as np, wave, os
sys.path.insert(0,'/tmp/claude-0/es/work')
from post2 import process
SR=44100
y=process('set600.wav.f32','set600_c.wav',0.75,0.0)
cuts=json.load(open('/tmp/claude-0/es/set/cuts.json'))
out='/tmp/claude-0/es/set/600'; os.makedirs(out,exist_ok=True)
def save(n,z):
    w=wave.open(f'{out}/{n}.wav','wb'); w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
    w.writeframes((np.clip(z,-1,1)*32767).astype(np.int16).tobytes()); w.close()
LOOPS={'Idle01':3.0,'IdleLoop':3.0,'OnLoop':2.2,'OffLoop':1.7}
for n,(s,ln) in cuts.items():
    a=int(s*SR)
    if n in LOOPS:
        L=int(LOOPS[n]*SR); xf=int(0.06*SR); a+=int(0.5*SR) if n!='Idle01' else 0
        seg=y[a:a+L+xf].copy()
        t=np.linspace(0,np.pi/2,xf)
        head=seg[:xf]*np.sin(t)+seg[L:L+xf]*np.cos(t)
        z=np.concatenate([head,seg[xf:L]])
    else:
        z=y[a:a+int(ln*SR)].copy()
        f=int(0.004*SR); z[:f]*=np.linspace(0,1,f); f2=int(0.03*SR); z[-f2:]*=np.linspace(1,0,f2)
    save(n,z)
    print(n, round(len(z)/SR,2), 'rms', round(float(np.sqrt((z*z).mean())),3))
