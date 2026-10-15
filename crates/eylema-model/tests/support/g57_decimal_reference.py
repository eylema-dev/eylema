"""Development-only synthetic Decimal80 oracle; no reference engine or model assets."""
from decimal import Decimal as D, getcontext
from pathlib import Path
import hashlib, json
getcontext().prec = 80
CASES = [[3,7,4,2,4,2,3],[5,6,2,1,2,1,3],[3,5,2,1,128,2,2]]
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

if __name__ == '__main__':
    import argparse
    p = argparse.ArgumentParser(); p.add_argument('--output', required=True); args = p.parse_args()
    output = {'protocol_sha256':'2824b75b5e24a47427d41ead074bc29981eb355b02f705f032eb11d41d45c747', 'cases':[fixture(c) for c in CASES]}
    Path(args.output).write_text(json.dumps(output,indent=2)+'\n')
