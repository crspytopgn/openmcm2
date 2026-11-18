"""Procedural realistic materials per tag (Cycles), used for preview and for baking."""
import bpy
def _n(nt, t, **kw):
    n = nt.nodes.new(t)
    for k, v in kw.items():
        if k.startswith('in_'): n.inputs[k[3:].replace('_', ' ')].default_value = v
        else: setattr(n, k, v)
    return n

def make(tag, livery=None):
    m = bpy.data.materials.new(tag); m.use_nodes = True; nt = m.node_tree; L = nt.links
    b = nt.nodes['Principled BSDF']
    tc = _n(nt, 'ShaderNodeTexCoord'); obj = tc.outputs['Object']
    def noise(scale, detail=8, rough=0.6, dist=0.0, src=None, w=None):
        n = _n(nt, 'ShaderNodeTexNoise', in_Scale=scale, in_Detail=detail, in_Roughness=rough)
        try: n.inputs['Distortion'].default_value = dist
        except Exception: pass
        L.new((src or obj), n.inputs['Vector']); return n.outputs['Fac']
    def ramp(fac, a, bcol, pa=0.0, pb=1.0):
        r = _n(nt, 'ShaderNodeValToRGB'); L.new(fac, r.inputs['Fac'])
        r.color_ramp.elements[0].position = pa; r.color_ramp.elements[1].position = pb
        r.color_ramp.elements[0].color = (*a, 1) if len(a) == 3 else a; r.color_ramp.elements[1].color = (*bcol, 1) if len(bcol) == 3 else bcol
        return r.outputs['Color']
    def maprange(fac, a, bb, fa=0.0, fb=1.0):
        r = _n(nt, 'ShaderNodeMapRange'); L.new(fac, r.inputs['Value']); r.inputs['From Min'].default_value = fa; r.inputs['From Max'].default_value = fb
        r.inputs['To Min'].default_value = a; r.inputs['To Max'].default_value = bb; return r.outputs['Result']
    def bump(h, strength=0.2, dist=0.001, normal=None):
        bp = _n(nt, 'ShaderNodeBump', in_Strength=strength, in_Distance=dist); L.new(h, bp.inputs['Height'])
        if normal: L.new(normal, bp.inputs['Normal'])
        L.new(bp.outputs['Normal'], b.inputs['Normal']); return bp
    def stretched(sx, sy, sz):
        mp = _n(nt, 'ShaderNodeMapping'); mp.inputs['Scale'].default_value = (sx, sy, sz); L.new(obj, mp.inputs['Vector']); return mp.outputs['Vector']
    b.inputs['Specular IOR Level'].default_value = 0.5
    def base(rgb): b.inputs['Base Color'].default_value = (*rgb, 1)
    def set_(rough, metal=0.0): b.inputs['Roughness'].default_value = rough; b.inputs['Metallic'].default_value = metal
    edge = None
    def edges():
        g = _n(nt, 'ShaderNodeNewGeometry'); return g.outputs['Pointiness']
    if tag in ('paint', 'tank'):
        if tag == 'tank': base((0.78, 0.76, 0.70)); set_(0.45)
        else:
            import os; base(tuple(map(float, os.environ.get('PAINT', '0.85,0.85,0.83').split(',')))); set_(0.32)
        b.inputs['Coat Weight'].default_value = 0.0
        # orange-peel / plastic grain
        bump(noise(900, 2, 0.5), 0.04, 0.0005)
        rr = maprange(noise(35, 6, 0.7), 0.26, 0.42, 0.3, 0.7); L.new(rr, b.inputs['Roughness'])
    elif tag == 'seat':
        base((0.025, 0.025, 0.028)); set_(0.62)
        # gripper ribs across the seat (along X) + fine grain
        w = _n(nt, 'ShaderNodeTexWave', wave_type='BANDS', bands_direction='Y', in_Scale=38.0, in_Distortion=0.0)
        L.new(obj, w.inputs['Vector'])
        n2 = noise(1500, 2, 0.5)
        mx = _n(nt, 'ShaderNodeMath', operation='ADD'); L.new(w.outputs['Fac'], mx.inputs[0]); L.new(n2, mx.inputs[1]); mx.inputs[1].default_value = 0
        bump(mx.outputs['Value'], 0.35, 0.002)
    elif tag in ('blackplastic', 'pad'):
        base((0.022, 0.022, 0.024)); set_(0.55 if tag == 'blackplastic' else 0.8); bump(noise(1200, 2, 0.5), 0.08, 0.0005)
    elif tag in ('alu', 'alu_cast', 'alu_anod', 'anod', 'anod2', 'silencer'):
        col = {'alu': (0.80, 0.80, 0.81), 'alu_cast': (0.62, 0.62, 0.63), 'alu_anod': (0.55, 0.10, 0.05), 'anod': (0.045, 0.045, 0.05), 'anod2': (0.05, 0.05, 0.06), 'silencer': (0.78, 0.78, 0.80)}[tag]
        base(col); set_(0.3, 1.0)
        if tag in ('alu', 'silencer'):
            # brushed: noise stretched along one axis
            v = stretched(600, 6, 6); f = noise(1.0, 4, 0.5, src=v)
            L.new(maprange(f, 0.22, 0.38, 0.35, 0.65), b.inputs['Roughness']); bump(f, 0.05, 0.0003)
        elif tag == 'alu_cast':
            f = noise(400, 6, 0.75); L.new(maprange(f, 0.42, 0.6), b.inputs['Roughness']); bump(f, 0.15, 0.0006)
        else:
            f = noise(250, 4, 0.6); L.new(maprange(f, 0.28, 0.4), b.inputs['Roughness'])
    elif tag in ('chrome', 'gold'):
        base((0.92, 0.92, 0.92) if tag == 'chrome' else (0.78, 0.58, 0.25)); set_(0.07 if tag == 'chrome' else 0.14, 1.0)
    elif tag in ('steel', 'steel_dark', 'disc', 'sprocket', 'chain'):
        col = {'steel': (0.62, 0.62, 0.64), 'steel_dark': (0.13, 0.13, 0.14), 'disc': (0.58, 0.58, 0.6), 'sprocket': (0.55, 0.55, 0.58), 'chain': (0.10, 0.09, 0.085)}[tag]
        base(col); set_(0.32, 1.0)
        if tag == 'disc':
            # radial turning marks: wave rings around X axis
            w = _n(nt, 'ShaderNodeTexWave', wave_type='RINGS', rings_direction='X', in_Scale=180.0); L.new(obj, w.inputs['Vector'])
            bump(w.outputs['Fac'], 0.05, 0.0002); L.new(maprange(w.outputs['Fac'], 0.22, 0.36), b.inputs['Roughness'])
        else:
            L.new(maprange(noise(300, 6, 0.7), 0.3, 0.5), b.inputs['Roughness'])
    elif tag in ('rubber', 'tyre', 'grip', 'hose'):
        base((0.028, 0.028, 0.028)); set_(0.85)
        f = noise(800, 3, 0.5); bump(f, 0.15, 0.0004)
        if tag == 'tyre':
            # faint dust in crevices
            ao = _n(nt, 'ShaderNodeAmbientOcclusion', in_Distance=0.02)
            dust = ramp(noise(30, 6, 0.6), (0.028, 0.027, 0.026), (0.10, 0.085, 0.07), 0.45, 0.9)
            mixn = _n(nt, 'ShaderNodeMix', data_type='RGBA'); mixn.inputs['Factor'].default_value = 0.0
            inv = _n(nt, 'ShaderNodeMath', operation='SUBTRACT'); inv.inputs[0].default_value = 1.0; L.new(ao.outputs['AO'], inv.inputs[1])
            mul = _n(nt, 'ShaderNodeMath', operation='MULTIPLY', in_Value=1.0); L.new(inv.outputs['Value'], mul.inputs[0]); mul.inputs[1].default_value = 0.8
            L.new(mul.outputs['Value'], mixn.inputs['Factor']); mixn.inputs['A'].default_value = (0.028, 0.028, 0.028, 1); L.new(dust, mixn.inputs['B'])
            L.new(mixn.outputs['Result'], b.inputs['Base Color'])
    elif tag in ('engine', 'engine_cover'):
        base((0.10, 0.10, 0.105) if tag == 'engine' else (0.20, 0.20, 0.21)); set_(0.45, 0.7)
        f = noise(350, 6, 0.75); bump(f, 0.12, 0.0005); L.new(maprange(f, 0.38, 0.6), b.inputs['Roughness'])
    elif tag == 'rad':
        base((0.16, 0.16, 0.17)); set_(0.55, 0.6)
        w = _n(nt, 'ShaderNodeTexWave', wave_type='BANDS', bands_direction='Z', in_Scale=600.0); L.new(obj, w.inputs['Vector']); bump(w.outputs['Fac'], 0.6, 0.001)
    elif tag == 'pipe':
        # heat-coloured steel: gold/blue/brown along the pipe (object Y), with blotchy noise
        set_(0.32, 1.0)
        sep = _n(nt, 'ShaderNodeSeparateXYZ'); L.new(obj, sep.inputs['Vector'])
        add = _n(nt, 'ShaderNodeMath', operation='ADD'); L.new(sep.outputs['Y'], add.inputs[0]); L.new(noise(8, 4, 0.6), add.inputs[1])
        r = _n(nt, 'ShaderNodeValToRGB'); L.new(add.outputs['Value'], r.inputs['Fac'])
        cr = r.color_ramp; cr.elements[0].position = 0.0; cr.elements[0].color = (0.40, 0.36, 0.32, 1); cr.elements[1].position = 1.4; cr.elements[1].color = (0.62, 0.48, 0.28, 1)
        e = cr.elements.new(0.55); e.color = (0.22, 0.24, 0.36, 1); e = cr.elements.new(0.85); e.color = (0.55, 0.40, 0.25, 1)
        L.new(r.outputs['Color'], b.inputs['Base Color']); bump(noise(300, 6, 0.7), 0.06, 0.0004)
    elif tag == 'pipe4':
        set_(0.28, 1.0); base((0.70, 0.62, 0.50))
    elif tag == 'spring':
        base((0.80, 0.60, 0.04)); set_(0.35)
    elif tag == 'caliper':
        base((0.62, 0.38, 0.08)); set_(0.35, 1.0)
    elif tag in ('kit', 'kit_dark'):
        import os; base(tuple(map(float, os.environ.get('KIT', '0.80,0.80,0.80').split(','))) if tag == 'kit' else (0.03, 0.03, 0.035)); set_(0.78)
        if tag == 'kit':
            import os
            c1 = (*map(float, os.environ.get('KIT', '0.80,0.80,0.80').split(',')), 1); c2 = (*map(float, os.environ.get('KIT2', '0.85,0.85,0.85').split(',')), 1); c3 = (*map(float, os.environ.get('KIT3', '0.03,0.03,0.035').split(',')), 1)
            at = _n(nt, 'ShaderNodeAttribute'); at.attribute_name = 'panel'
            sp = _n(nt, 'ShaderNodeSeparateColor'); L.new(at.outputs['Color'], sp.inputs['Color'])
            m1 = _n(nt, 'ShaderNodeMix'); m1.data_type = 'RGBA'; m1.inputs[6].default_value = c1; m1.inputs[7].default_value = c2; L.new(sp.outputs[1], m1.inputs[0])
            m2 = _n(nt, 'ShaderNodeMix'); m2.data_type = 'RGBA'; L.new(m1.outputs[2], m2.inputs[6]); m2.inputs[7].default_value = c3; L.new(sp.outputs[2], m2.inputs[0])
            L.new(m2.outputs[2], b.inputs['Base Color'])
        # woven fabric: fine crossed waves
        w1 = _n(nt, 'ShaderNodeTexWave', wave_type='BANDS', bands_direction='X', in_Scale=900.0); L.new(obj, w1.inputs['Vector'])
        w2 = _n(nt, 'ShaderNodeTexWave', wave_type='BANDS', bands_direction='Z', in_Scale=900.0); L.new(obj, w2.inputs['Vector'])
        mx = _n(nt, 'ShaderNodeMath', operation='MULTIPLY'); L.new(w1.outputs['Fac'], mx.inputs[0]); L.new(w2.outputs['Fac'], mx.inputs[1])
        bump(mx.outputs['Value'], 0.12, 0.0004)
    elif tag == 'protector':
        import os; base(tuple(map(float, os.environ.get('KIT', '0.80,0.80,0.80').split(',')))); set_(0.35)
    elif tag in ('helmet',):
        import os; base(tuple(map(float, os.environ.get('HELMET', '0.85,0.85,0.84').split(',')))); set_(0.22); bump(noise(1000, 2, 0.5), 0.02, 0.0003)
    elif tag in ('helmet_trim', 'goggle', 'strap', 'brace', 'boot_strap', 'boot_trim', 'sole', 'glove_trim', 'buckle'):
        col = {'helmet_trim': (0.03, 0.03, 0.03), 'goggle': tuple(map(float, __import__('os').environ.get('GOGGLE', '0.03,0.03,0.035').split(','))), 'strap': (0.04, 0.04, 0.045), 'brace': (0.05, 0.05, 0.055), 'boot_strap': (0.04, 0.04, 0.045), 'boot_trim': (0.05, 0.05, 0.055), 'sole': (0.03, 0.03, 0.03), 'glove_trim': (0.04, 0.04, 0.045), 'buckle': (0.6, 0.6, 0.62)}[tag]
        base(col); set_(0.55 if tag != 'buckle' else 0.3, 1.0 if tag == 'buckle' else 0.0); bump(noise(900, 2, 0.5), 0.08, 0.0004)
    elif tag == 'lens':
        base((0.02, 0.02, 0.025)); set_(0.05, 0.6)
    elif tag in ('boot', 'glove'):
        base((0.75, 0.75, 0.74) if tag == 'boot' else (0.72, 0.72, 0.72)); set_(0.55 if tag == 'boot' else 0.75)
        bump(noise(700, 4, 0.6), 0.10, 0.0005)
    elif tag == 'plate':
        base((0.92, 0.92, 0.9)); set_(0.4)
    else:
        base((1, 0, 1)); set_(0.5)
    return m

def apply(objs):
    cache = {}
    for o in objs:
        t = o.get('tag', 'paint')
        if t == 'keep': continue
        if t not in cache: cache[t] = make(t)
        o.data.materials.clear(); o.data.materials.append(cache[t])
