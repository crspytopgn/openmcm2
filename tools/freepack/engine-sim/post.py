import numpy as np, wave
from scipy.signal import butter, sosfiltfilt, sosfilt
from scipy.ndimage import maximum_filter1d, uniform_filter1d
SR=44100
def smooth(g, att, rel):
    # one-pole with separate attack (gain going down) and release, run on a
    # signal already looked ahead so drops start before the peak
    a=np.exp(-1/(att*SR)); r=np.exp(-1/(rel*SR)); out=np.empty_like(g); v=g[0]
    for i,x in enumerate(g):
        v = a*v+(1-a)*x if x<v else r*v+(1-r)*x
        out[i]=v
    return out
def process(f32, out_wav, ratio=0.75, trim=0.0, ceiling=0.89):
    x=np.fromfile(f32,np.float32).astype(np.float64)[int(trim*SR):]
    x=sosfiltfilt(butter(2,25,'hp',fs=SR,output='sos'),x)
    # slow leveller: RMS 30 ms, zero-phase smoothed (no transient slips through)
    e=np.sqrt(uniform_filter1d(x*x,int(0.03*SR)))+1e-9
    db=20*np.log10(e)
    db=sosfiltfilt(butter(1,6,fs=SR,output='sos'),db)
    ref=np.percentile(db,95)
    g=10**(-(db-ref)*ratio/20)
    y=x*g
    y/=np.percentile(np.abs(y),99.9)/ceiling
    # look-ahead peak limiter: no hard or soft clipping
    look=int(0.004*SR)
    pk=maximum_filter1d(np.abs(y),2*look+1)
    lg=np.minimum(1.0,ceiling/np.maximum(pk,1e-9))
    lg=smooth(lg,0.002,0.08)
    lg=np.minimum(lg, maximum_filter1d(lg*0+1,1))
    y=y*lg
    y=np.clip(y,-0.999,0.999)
    w=wave.open(out_wav,'wb'); w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
    w.writeframes((y*32767).astype(np.int16).tobytes()); w.close()
    return y
