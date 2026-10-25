import sys,re,struct
import os; S=os.environ.get('MM2_RE_DIR','.')  # dir holding out/ and fn/ from DumpAll
data=open('/mnt/user-data/uploads/mm2/GAME/Mcm2.exe','rb').read()
# section map
import pefile
pe=pefile.PE(data=data,fast_load=True)
def rd(va,n):
    rva=va-0x400000
    for s in pe.sections:
        if s.VirtualAddress<=rva<s.VirtualAddress+s.SizeOfRawData:
            o=s.PointerToRawData+rva-s.VirtualAddress; return data[o:o+n]
    return None
def fmt(m):
    a=int(m.group(2),16); b=rd(a,8)
    if b is None or len(b)<8: return m.group(0)
    f=struct.unpack('<f',b[:4])[0]; d=struct.unpack('<d',b)[0]; i=struct.unpack('<i',b[:4])[0]
    s = '%g'%f if (abs(f)<1e9 and (abs(f)>1e-6 or f==0)) else ('i%d'%i)
    if (abs(f)>1e9 or (f!=0 and abs(f)<1e-30)) and 1e-6<abs(d)<1e9: s+=' d%g'%d
    return m.group(0)+'{'+s+'}'
def hexf(m):
    v=int(m.group(1),16)
    if 0x3a000000<=v<=0x4f000000 or 0xba000000<=v<=0xcf000000:
        return m.group(0)+'{%g}'%struct.unpack('<f',struct.pack('<I',v))[0]
    return m.group(0)
for a in sys.argv[1:]:
    t=open(f'{S}/fn/{a.zfill(8)}.c').read()
    t=re.sub(r'(_?DAT_)([0-9a-f]{8})',fmt,t)
    t=re.sub(r'\b0x([0-9a-f]{8})\b',hexf,t)
    # drop local var decl block
    t=re.sub(r'\n  (?:[\w\s\*]+?) [\w\*\[\]]+;(?=\n)','',t)
    print(t)
