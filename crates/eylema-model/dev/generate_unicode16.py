"""Own deterministic Unicode16 data generator; no runtime dependency."""
import argparse, hashlib, json, struct, unicodedata as ud
from pathlib import Path
PROPERTY='3627a624f63370f0e726dde726f42934c8ca898c5e18fb69abdeee5a27ff00d8'
COMPOSITION='e83834fdad296e46779fb702e11dd9b8686409a6346461d02fe3a22afaf26dd4'
def generate():
    assert ud.unidata_version=='16.0.0','Wrong Unicode development data'
    h=hashlib.sha256(b'g29-unicode-data.v1\0'+b'16.0.0\0');groups=[];ccc=[];decomp=[];comps=[];folds=[]
    start=prev=0;last=None
    for cp in range(0x110000):
        if 0xd800<=cp<=0xdfff:continue
        ch=chr(cp);cat=ud.category(ch);cc=ud.combining(ch);d=ud.decomposition(ch).encode('ascii');f=ch.casefold().encode()
        h.update(struct.pack('>I',cp)+cat.encode()+struct.pack('>HH',cc,len(d))+d+struct.pack('>H',len(f))+f)
        if cat!=last or cp!=prev+1:
            if last is not None:groups.append((start,prev,last))
            start=cp;last=cat
        prev=cp
        if cc:ccc.append((cp,cc))
        if d and not d.startswith(b'<'):
            values=[int(x,16) for x in d.split()];decomp.append((cp,values))
            if len(values)==2 and ud.normalize('NFC',''.join(map(chr,values)))==ch:comps.append((*values,cp))
        folded=ch.casefold()
        if len(folded)==1 and folded in 'strev mld'.replace(' ',''):folds.append((cp,ord(folded)))
    groups.append((start,prev,last));comps.sort()
    hc=hashlib.sha256(b'g29-unicode-composition.v1\0'+b'16.0.0\0')
    for row in comps:hc.update(struct.pack('>III',*row))
    assert h.hexdigest()==PROPERTY,('Property source mismatch',h.hexdigest())
    assert hc.hexdigest()==COMPOSITION,('Composition source mismatch',hc.hexdigest())
    out=bytearray(b'EYLUNIC16'+struct.pack('<IIIIII',1,len(groups),len(ccc),len(decomp),len(comps),len(folds)))
    for a,b,c in groups:out.extend(struct.pack('<II',a,b)+c.encode())
    for cp,cc in ccc:out.extend(struct.pack('<IH',cp,cc))
    for cp,seq in decomp:
        out.extend(struct.pack('<IH',cp,len(seq)))
        for v in seq:out.extend(struct.pack('<I',v))
    for row in comps:out.extend(struct.pack('<III',*row))
    for cp,fold in folds:out.extend(struct.pack('<IB',cp,fold))
    return bytes(out),{'format':'g29-unicode16-data.v1','unicode':'16.0.0','property_source_sha256':PROPERTY,'composition_source_sha256':COMPOSITION,'binary_sha256':hashlib.sha256(out).hexdigest(),'binary_bytes':len(out),'counts':list(map(len,[groups,ccc,decomp,comps,folds])),'provenance':'Derived by authored stdlib-only generator from CPython3.14 unicodedata16.0.0; data under accompanying Unicode LicenseV3; own algorithms Apache-2.0.','runtime_engine':'none'}
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--output',required=True);args=ap.parse_args();data,info=generate();p=Path(args.output);p.mkdir(parents=True,exist_ok=True);(p/'unicode16.bin').write_bytes(data);(p/'unicode16.provenance.json').write_text(json.dumps(info,indent=2)+'\n');print(json.dumps(info))
if __name__=='__main__':main()
