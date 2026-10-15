"""Development-only synthetic Decimal80 oracle; no reference engine or model assets."""
from decimal import Decimal as D, getcontext
from pathlib import Path
import hashlib, json
getcontext().prec = 80
ROOT = Path(__file__).parent
CASES = json.loads((ROOT/'protocol.json').read_text())['cases']
EPS = D('0.000001')

def value(i, salt):
    return D(((i * 17 + salt * 13) % 31) - 15) / D(64)

def norm(x, salt):
    inv = (sum(v*v for v in x)/D(len(x)) + EPS).sqrt()
    return [v / inv * (D(1)+value(i, salt)/D(4)) for i,v in enumerate(x)]

def linear(x, rows, salt):
    return [sum(value(r*len(x)+c,salt)*v for c,v in enumerate(x)) for r in range(rows)]

def trig(x):
    # Small positive angles in the frozen fixtures; Decimal Taylor, not platform libm.
    s, c, st, ct = x, D(1), x, D(1)
    for n in range(1,100):
        st *= -x*x/D((2*n)*(2*n+1))
        ct *= -x*x/D((2*n-1)*(2*n))
        s += st; c += ct
        if abs(st)+abs(ct)<D('1e-76'):
            return s,c
    raise AssertionError('series bound')

def rope(x, pos):
    out = x.copy(); half = len(x)//2
    for j in range(half):
        frequency = (-D(2*j)/D(len(x))*D(1000000).ln()).exp()
        s,c = trig(D(pos)*frequency)
        out[j] = x[j]*c-x[j+half]*s
        out[j+half] = x[j+half]*c+x[j]*s
    return out

def fixture(case):
    h, inter, qh, kh, width, layers, tokens = case
    # Batch all time positions at each layer. Candidate will instead advance tokens.
    states = [[value(t*h+i, 2) for i in range(h)] for t in range(tokens)]
    cache = []
    for layer in range(layers):
        seed = 11+layer*19
        normalized = [norm(x,seed) for x in states]
        queries = [linear(x,qh*width,seed+1) for x in normalized]
        keys = [linear(x,kh*width,seed+2) for x in normalized]
        vals = [linear(x,kh*width,seed+3) for x in normalized]
        for t in range(tokens):
            for a in range(qh):
                queries[t][a*width:(a+1)*width] = rope(norm(queries[t][a*width:(a+1)*width],seed+4),t)
            for a in range(kh):
                keys[t][a*width:(a+1)*width] = rope(norm(keys[t][a*width:(a+1)*width],seed+5),t)
        updated = []
        for t in range(tokens):
            concatenated = []
            for a in range(qh):
                b = a//(qh//kh)
                dots = [sum(queries[t][a*width+j]*keys[s][b*width+j] for j in range(width))/D(width).sqrt() for s in range(t+1)]
                maximum = max(dots)
                exponentials = [(v-maximum).exp() for v in dots]
                total = sum(exponentials)
                concatenated.extend(sum(exponentials[s]/total*vals[s][b*width+j] for s in range(t+1)) for j in range(width))
            projected = linear(concatenated,h,seed+6)
            residual = [a+b for a,b in zip(states[t],projected)]
            post = norm(residual,seed+7)
            gate, up = linear(post,inter,seed+8),linear(post,inter,seed+9)
            hidden = [g/(D(1)+(-g).exp())*u for g,u in zip(gate,up)]
            down = linear(hidden,h,seed+10)
            updated.append([a+b for a,b in zip(residual,down)])
        states = updated
        cache.extend(v for row in keys for v in row)
        cache.extend(v for row in vals for v in row)
    final = [norm(x,3) for x in states]
    logits = [linear(x,7,2) for x in final]  # Same synthetic embedding row rule.
    flattened = [v for row in final for v in row] + [v for row in logits for v in row] + cache
    winners = [max(range(7),key=lambda i: (row[i],-i)) for row in logits]
    return {'values':[str(v) for v in flattened], 'greedy':winners, 'count':len(flattened)}

def generate():
    return {'protocol_sha256':hashlib.sha256((ROOT/'protocol.json').read_bytes()).hexdigest(),'cases':[fixture(c) for c in CASES]}

if __name__ == '__main__':
    output=generate()
    print(json.dumps({'cases':len(output['cases']),'counts':[c['count'] for c in output['cases']]}))

# Independent F64, single-position reference. This code never imports product kernels.
import math
import struct
import time

_BF16 = tuple(struct.unpack('<f', struct.pack('<I', n << 16))[0] for n in range(65536))

def f64_dot(row, vector):
    if len(row) != len(vector):
        raise ValueError('row shape')
    result = 0.0
    for bits, x in zip(row, vector):
        v = _BF16[bits]
        if not math.isfinite(v):
            raise ValueError('nonfinite stored value')
        result += v * x
    if not math.isfinite(result):
        raise ValueError('nonfinite reduction')
    return result

def pretrained_reference(weight_path, output_path):
    import mmap, os, resource, sys
    if sys.byteorder != 'little':
        raise ValueError('reference host endianness')
    began = time.monotonic()
    def checkpoint():
        if time.monotonic()-began >= 600:
            raise TimeoutError('reference deadline')
    expected_sha = 'f47f71177f32bcd101b7573ec9171e6a57f4f4d31148d38e382306f42996874b'
    digest = hashlib.sha256()
    with open(weight_path, 'rb') as source:
        if os.fstat(source.fileno()).st_size != 1503300328:
            raise ValueError('artifact size')
        while block := source.read(65536):
            digest.update(block); checkpoint()
        if digest.hexdigest() != expected_sha:
            raise ValueError('artifact hash')
        with mmap.mmap(source.fileno(), 0, access=mmap.ACCESS_READ) as mapped:
            header_size = struct.unpack_from('<Q', mapped)[0]
            if not 0 < header_size <= 262144:
                raise ValueError('header limit')
            header = json.loads(mapped[8:8+header_size])
            expected = {'model.embed_tokens.weight':[151936,1024], 'lm_head.weight':[151936,1024], 'model.norm.weight':[1024]}
            for layer in range(28):
                prefix = f'model.layers.{layer}.'
                for suffix, shape in [('input_layernorm.weight',[1024]),('post_attention_layernorm.weight',[1024]),
                    ('self_attn.q_proj.weight',[2048,1024]),('self_attn.k_proj.weight',[1024,1024]),('self_attn.v_proj.weight',[1024,1024]),
                    ('self_attn.o_proj.weight',[1024,2048]),('self_attn.q_norm.weight',[128]),('self_attn.k_norm.weight',[128]),
                    ('mlp.gate_proj.weight',[3072,1024]),('mlp.up_proj.weight',[3072,1024]),('mlp.down_proj.weight',[1024,3072])]:
                    expected[prefix+suffix] = shape
            if set(header)-{'__metadata__'} != set(expected):
                raise ValueError('complete tensor map')
            ranges=[]
            for name,shape in expected.items():
                entry=header[name]
                if entry['dtype']!='BF16' or entry['shape']!=shape:
                    raise ValueError('tensor specification')
                a,b=entry['data_offsets']
                if not (type(a) is int and type(b) is int and 0<=a<b<=len(mapped)-8-header_size and b-a==2*math.prod(shape)):
                    raise ValueError('tensor extent')
                ranges.append((a,b))
            ranges.sort()
            if ranges[0][0]!=0 or ranges[-1][1]!=len(mapped)-8-header_size or any(a[1]!=b[0] for a,b in zip(ranges,ranges[1:])):
                raise ValueError('tensor coverage')
            def values(name):
                a,b=header[name]['data_offsets']
                return memoryview(mapped)[8+header_size+a:8+header_size+b].cast('H')
            def linear(vector,name):
                row_count,width=expected[name]
                if len(vector)!=width: raise ValueError('input width')
                weights=values(name)
                out=[]
                for row in range(row_count):
                    if row%64==0: checkpoint()
                    out.append(f64_dot(weights[row*width:(row+1)*width],vector))
                weights.release()
                return out
            def rms(vector,name):
                weights=values(name)
                if len(weights)!=len(vector): raise ValueError('norm width')
                scale=1/math.sqrt(sum(x*x for x in vector)/len(vector)+1e-6)
                out=[x*scale*_BF16[w] for x,w in zip(vector,weights)]
                weights.release()
                if not all(math.isfinite(x) for x in out): raise ValueError('norm finite')
                return out
            # At position zero rotation is the identity, and the single visible
            # key has softmax probability one. Multi-position semantics are
            # independently pinned by the Decimal80 fixtures above.
            embed=values('model.embed_tokens.weight')
            state=[_BF16[v] for v in embed[9707*1024:9708*1024]]
            embed.release()
            layers=[]
            for layer in range(28):
                checkpoint(); prefix=f'model.layers.{layer}.'
                x=rms(state,prefix+'input_layernorm.weight')
                query=linear(x,prefix+'self_attn.q_proj.weight')
                key=linear(x,prefix+'self_attn.k_proj.weight')
                value=linear(x,prefix+'self_attn.v_proj.weight')
                for head in range(16):
                    query[head*128:(head+1)*128]=rms(query[head*128:(head+1)*128],prefix+'self_attn.q_norm.weight')
                for head in range(8):
                    key[head*128:(head+1)*128]=rms(key[head*128:(head+1)*128],prefix+'self_attn.k_norm.weight')
                attended=[v for head in range(16) for v in value[(head//2)*128:(head//2+1)*128]]
                projected=linear(attended,prefix+'self_attn.o_proj.weight')
                state=[a+b for a,b in zip(state,projected)]
                post=rms(state,prefix+'post_attention_layernorm.weight')
                gate=linear(post,prefix+'mlp.gate_proj.weight'); up=linear(post,prefix+'mlp.up_proj.weight')
                # Stable SiLU avoids overflow for large negative inputs.
                hidden=[(g/(1+math.exp(-g)) if g>=0 else g*math.exp(g)/(1+math.exp(g)))*u for g,u in zip(gate,up)]
                state=[a+b for a,b in zip(state,linear(hidden,prefix+'mlp.down_proj.weight'))]
                layers.append(state.copy())
            logits=linear(rms(state,'model.norm.weight'),'model.embed_tokens.weight')
            result={'format':'g30.f64-reference.v1','artifact_sha256':expected_sha,'token':9707,'per_layer_hidden':layers,'logits':logits,
                'greedy':max(range(len(logits)),key=lambda i:(logits[i],-i)), 'elapsed_seconds':time.monotonic()-began,
                'process_peak_rss_bytes':resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
                'logical_reference_limit_bytes':268435456,'mapped_pages_excluded_from_logical_limit':True}
            raw=(json.dumps(result,allow_nan=False)+'\n').encode()
            if len(raw)>16*1024*1024: raise ValueError('reference output limit')
            fd=os.open(output_path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
            with os.fdopen(fd,'wb') as output: output.write(raw)
    return {'greedy':result['greedy'],'layers':len(layers),'logits':len(logits)}
