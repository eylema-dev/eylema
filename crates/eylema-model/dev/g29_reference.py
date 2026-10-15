"""Owned stdlib-only independent G29 reference, never a production engine."""
import argparse, hashlib, json, re, sys, unicodedata as ud
from pathlib import Path

def alphabet():
    kept=list(range(33,127))+list(range(161,173))+list(range(174,256))
    result={b:chr(b) for b in kept}
    for n,b in enumerate(b for b in range(256) if b not in result):result[b]=chr(256+n)
    return [result[b] for b in range(256)]

def bpe(text,ranks):
    parts=list(text)
    while len(parts)>1:
        pairs=[(ranks[p],p) for p in zip(parts,parts[1:]) if p in ranks]
        if not pairs:break
        _,best=min(pairs)
        new=[];i=0
        while i<len(parts):
            if i+1<len(parts) and (parts[i],parts[i+1])==best:new.append(parts[i]+parts[i+1]);i+=2
            else:new.append(parts[i]);i+=1
        parts=new
    return parts

def pattern():
    assert ud.unidata_version=='16.0.0'
    groups={'L':[],'N':[]}
    for cp in range(0x110000):
        cat=ud.category(chr(cp))[0]
        if cat in groups:groups[cat].append(cp)
    def cls(points):
        spans=[];start=prev=points[0]
        for cp in points[1:]:
            if cp!=prev+1:spans.append((start,prev));start=cp
            prev=cp
        spans.append((start,prev))
        def escaped(cp):return '\\U%08x'%cp
        return ''.join(escaped(a)+(('-'+escaped(b)) if a!=b else '') for a,b in spans)
    letter=cls(groups['L']);number=cls(groups['N'])
    white=r'\u0009-\u000d\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000'
    return re.compile("(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\\r\\n"+letter+number+"]?["+letter+"]+|["+number+"]| ?[^"+white+letter+number+"]+[\\r\\n]*|["+white+"]*[\\r\\n]+|["+white+"]+(?![^"+white+"])|["+white+"]+")

def render(messages):
    return ''.join('<|im_start|>'+role+'\n'+text+'<|im_end|>\n' for role,text in messages)+'<|im_start|>assistant\n<think>\n\n</think>\n\n'

def generate(tokenizer,config):
    raw=Path(tokenizer).read_bytes();cfgraw=Path(config).read_bytes()
    assert len(raw)==11422654 and hashlib.sha256(raw).hexdigest()=='aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4'
    assert hashlib.sha256(cfgraw).hexdigest()=='d5d09f07b48c3086c508b30d1c9114bd1189145b74e982a265350c923acd8101'
    obj=json.loads(raw);cfg=json.loads(cfgraw)
    assert hashlib.sha256(cfg['chat_template'].encode()).hexdigest()=='a55ee1b1660128b7098723e0abcd92caa0788061051c62d51cbe87d9cf1974d8'
    vocab=obj['model']['vocab'];ranks={tuple(p):i for i,p in enumerate(obj['model']['merges'])};a=alphabet();rx=pattern()
    added={v['content']:v['id'] for v in obj['added_tokens']}
    def ordinary(s):
        norm=ud.normalize('NFC',s);pieces=rx.findall(norm)
        assert ''.join(pieces)==norm
        ids=[]
        for piece in pieces:
            encoded=''.join(a[b] for b in piece.encode())
            ids.extend(vocab[p] for p in bpe(encoded,ranks))
        return norm,pieces,ids
    cases=[]
    texts=['','Hello world',' Hello world',"I'M we're can't 'ſ",'İstanbul ı I ÇĞŞ','C\u0327G\u0306S\u0327','q\u0307\u0323','가','你好世界','🙂🚀𐐀','123456',' \t Hello\r\nworld  \n\t','abababa']
    for i,s in enumerate(texts):
        norm,pieces,ids=ordinary(s)
        cases.append({'name':'text_%02d'%i,'input_hex':s.encode().hex(),'normalized_hex':norm.encode().hex(),'pieces_hex':[p.encode().hex() for p in pieces],'ids':ids,'decoded_hex':norm.encode().hex()})
    assert cases[1]['ids']==[9707,1879], 'Published Qwen Hello-world anchor mismatch'
    prompts=[]
    for i,messages in enumerate([[('user','Hi')],[('system','Be precise.'),('user','İstanbul?')],[('user','One'),('assistant','Two'),('user','Three')],[('system','Be precise.'),('user','One'),('assistant','Two'),('user','Three')], [('user','')], [('user','\n')], [('user','\r\n')], [('user',' \n')], [('user','\t')], [('user','\nİstanbul')], [('user','\u0327')], [('system','\n'),('user','\r\n')], [('user','One'),('assistant','\nTwo'),('user','\nThree')], [('user',' \r\n')]]):
        rendered=render(messages);ids=[];at=0
        # Independent segmentation of explicitly rendered structure into added-token literals.
        tokens=sorted(added,key=len,reverse=True)
        while at<len(rendered):
            found=next((t for t in tokens if rendered.startswith(t,at)),None)
            if found:ids.append(added[found]);at+=len(found);continue
            end=min([rendered.find(t,at) for t in tokens if rendered.find(t,at)>=0]+[len(rendered)])
            ids.extend(ordinary(rendered[at:end])[2]);at=end
        prompts.append({'name':'prompt_%02d'%i,'messages':[{'role':r,'content_hex':s.encode().hex()} for r,s in messages],'rendered_hex':rendered.encode().hex(),'ids':ids})
    return {'format':'g29-tokenizer-reference.v1','unicode':'16.0.0','source':'c1899de289a04d12100db370d81485cdf75e47ca','tokenizer_sha256':hashlib.sha256(raw).hexdigest(),'config_sha256':hashlib.sha256(cfgraw).hexdigest(),'template_sha256':hashlib.sha256(cfg['chat_template'].encode()).hexdigest(),'cases':cases,'prompts':prompts,'added':obj['added_tokens'],'byte_ids':[vocab[x] for x in a]}

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--self-test',action='store_true');ap.add_argument('--tokenizer');ap.add_argument('--config');ap.add_argument('--output');args=ap.parse_args()
    if args.self_test:
        import unittest
        import test_g29_reference
        ok=unittest.TextTestRunner().run(unittest.defaultTestLoader.loadTestsFromModule(test_g29_reference)).wasSuccessful()
        raise SystemExit(0 if ok else 1)
    out=generate(args.tokenizer,args.config);Path(args.output).write_text(json.dumps(out,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'reference_cases':len(out['cases']),'prompt_cases':len(out['prompts']),'output_sha256':hashlib.sha256(Path(args.output).read_bytes()).hexdigest()}))
if __name__=='__main__':main()
