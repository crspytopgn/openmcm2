import sys
import os; S=os.environ.get('MM2_RE_DIR','.')  # dir holding out/ and fn/ from DumpAll
cal={};cr={}
for l in open(S+'/out/callgraph.tsv'):
    a,b=l.rstrip('\n').split('\t'); a=a.lstrip('0'); cal[a]=[x.lstrip('0') for x in b.split()]
    for c in cal[a]: cr.setdefault(c,[]).append(a)
sz={l.split('\t')[0].lstrip('0'):l.split('\t')[2].strip() for l in open(S+'/out/functions.tsv')}
vt={}
for l in open(S+'/out/vtables.tsv'):
    t,s,e=l.rstrip('\n').split('\t')
    for i,x in enumerate(e.split()): vt.setdefault(x,[]).append('%s[%d]'%(t,i))
cls=dict(l.split() for l in open(S+'/cls.txt'))
def tag(a): 
    v=[x for x in vt.get(a,[]) if x.split('[')[0] in cls]
    return a+'('+sz.get(a,'?')+(' vt:'+','.join(cls[x.split('[')[0]]+'['+x.split('[')[1] for x in v) if v else '')+')'
for a in sys.argv[1:]:
    a=a.lstrip('0')
    print(tag(a)); print('  callers:',' '.join(tag(x) for x in cr.get(a,[]))); print('  callees:',' '.join(tag(x) for x in cal.get(a,[])))
