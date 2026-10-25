import struct,sys
class R:
    def __init__(s,d): s.d=d; s.o=0
    def f(s): v=struct.unpack_from('<f',s.d,s.o)[0]; s.o+=4; return v
    def u(s): v=struct.unpack_from('<I',s.d,s.o)[0]; s.o+=4; return v
    def h(s): v=struct.unpack_from('<H',s.d,s.o)[0]; s.o+=2; return v
def tri_tree(r,st):
    k=r.f()
    if k>=0:
        bb=[r.f() for _ in range(6)]; st['in']+=1; tri_tree(r,st); tri_tree(r,st)
    else:
        d=r.f(); n=(r.f(),r.f(),r.f()); idx=(r.h(),r.h(),r.h()); st['leaf']+=1; st.setdefault('ex',[]).append((k,n,d,idx))
def tree2(r,st):
    k=r.f()
    if k>=0:
        bb=[r.f() for _ in range(6)]; st['in2']+=1; tree2(r,st); tree2(r,st)
    else:
        p=(r.f(),r.f(),r.f()); st['leaf2']+=1; st.setdefault('ex2',[]).append((k,p))
def body(r):
    m=[r.f() for _ in range(12)]
    a=r.u(); has=r.u(); st={'in':0,'leaf':0,'in2':0,'leaf2':0,'m':m,'a':a}
    if has:
        n=r.u(); st['nv']=n; st['v0']=[(r.f(),r.f(),r.f()) for _ in range(n)][:2]
        tri_tree(r,st)
    h2=r.u()
    if h2: tree2(r,st)
    return st
d=open(sys.argv[1],'rb').read(); r=R(d)
t=r.u()
if t==0:
    st=body(r)
else:
    n=r.u(); x=r.u(); st={'type':1,'n':n,'x':x}
    for i in range(n):
        parent=r.u(); b=body(r); print('part',i,'parent',parent,{k:v for k,v in b.items() if k not in('ex','ex2')})
print('end',r.o,len(d)); print({k:(v if k not in('ex','ex2') else v[:3]) for k,v in st.items()})
