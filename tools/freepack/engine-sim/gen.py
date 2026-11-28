import sys,subprocess,os
def render(name, sched, IDLEPOS=0.985, OUTLET=800, PIPE=30, IR='mild_exhaust_0', tmpl='mx600.tmpl', extra=None):
    s=open(tmpl).read().replace('IDLEPOS',str(IDLEPOS)).replace('OUTLET',str(OUTLET)).replace('PIPE',str(PIPE)).replace('IR',IR) if False else open(tmpl).read()
    for k,v in dict(IDLEPOS=IDLEPOS,OUTLET=OUTLET,PIPE=PIPE).items(): s=s.replace(k,str(v))
    s=s.replace('ir_lib.IR','ir_lib.'+IR)
    for a,b in (extra or {}).items(): s=s.replace(a,b)
    open(f'{name}.engine.mr','w').write(s)
    open(f'{name}.main.mr','w').write(f'import "engine_sim.mr"\nimport "/tmp/claude-0/es/work/{name}.engine.mr"\nmain()\n')
    open(f'{name}.sched','w').write(''.join(' '.join(map(str,(x[0],x[1],x[2],x[3] if len(x)>4 else x[2],x[-1])))+'\n' for x in sched))
    r=subprocess.run(['/tmp/claude-0/es/render/build3/es-render',f'/tmp/claude-0/es/work/{name}.main.mr',f'/tmp/claude-0/es/work/{name}.wav',f'/tmp/claude-0/es/work/{name}.sched'],cwd='/tmp/claude-0/es/repo/assets',capture_output=True,text=True)
    log=r.stderr
    if 'wrote' not in log: print(log[-2000:]); print(open('/tmp/claude-0/es/repo/assets/error_log.log').read()[:2000] if os.path.exists('/tmp/claude-0/es/repo/assets/error_log.log') else '')
    return log
if __name__=='__main__':
    log=render('t1',[(3,0,0,-1),(2,1,0,-1),(2,0,0,-1),(1,0.3,0,-1),(1,0,0,-1),(3,1,6000,0),(3,0,6000,0)])
    print('\n'.join(l for l in log.splitlines() if l.startswith('t=') or 'IR' in l or 'wrote' in l))
