// Independent first-party G68 oracle, adapted from the separately accepted G57 F64 oracle.
// No product imports/dependencies; real path returns normalized final hidden states only.
use std::{collections::BTreeMap, io::Read};
type R<T> = Result<T, String>;
#[derive(Debug)]
enum J {
    S(String),
    N(usize),
    A(Vec<J>),
    O(BTreeMap<String, J>),
}
impl J {
    fn obj(&self) -> R<&BTreeMap<String, J>> {
        if let Self::O(v) = self {
            Ok(v)
        } else {
            Err("object".into())
        }
    }
    fn arr(&self) -> R<&[J]> {
        if let Self::A(v) = self {
            Ok(v)
        } else {
            Err("array".into())
        }
    }
    fn num(&self) -> R<usize> {
        if let Self::N(v) = self {
            Ok(*v)
        } else {
            Err("integer".into())
        }
    }
    fn text(&self) -> R<&str> {
        if let Self::S(v) = self {
            Ok(v)
        } else {
            Err("string".into())
        }
    }
}
struct Parser<'a> {
    b: &'a [u8],
    at: usize,
    nodes: usize,
}
impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.b.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }
    fn value(&mut self, depth: usize) -> R<J> {
        self.ws();
        self.nodes += 1;
        if depth > 8 || self.nodes > 20000 {
            return Err("JSON bound".into());
        }
        let c = *self.b.get(self.at).ok_or("JSON EOF")?;
        self.at += 1;
        match c {
            b'"' => {
                let start = self.at;
                while let Some(&c) = self.b.get(self.at) {
                    if c == b'"' {
                        let s = std::str::from_utf8(&self.b[start..self.at])
                            .map_err(|_| "UTF8")?
                            .to_owned();
                        self.at += 1;
                        return Ok(J::S(s));
                    }
                    if c == b'\\' || c < 32 || self.at - start > 512 {
                        return Err("string subset".into());
                    }
                    self.at += 1;
                }
                Err("string EOF".into())
            }
            b'[' => {
                let mut a = Vec::new();
                self.ws();
                if self.b.get(self.at) == Some(&b']') {
                    self.at += 1;
                    return Ok(J::A(a));
                }
                loop {
                    a.push(self.value(depth + 1)?);
                    self.ws();
                    let c = *self.b.get(self.at).ok_or("array EOF")?;
                    self.at += 1;
                    if c == b']' {
                        break;
                    }
                    if c != b',' {
                        return Err("array separator".into());
                    }
                }
                Ok(J::A(a))
            }
            b'{' => {
                let mut o = BTreeMap::new();
                self.ws();
                if self.b.get(self.at) == Some(&b'}') {
                    self.at += 1;
                    return Ok(J::O(o));
                }
                loop {
                    let k = self.value(depth + 1)?.text()?.to_owned();
                    self.ws();
                    if self.b.get(self.at) != Some(&b':') {
                        return Err("colon".into());
                    }
                    self.at += 1;
                    let v = self.value(depth + 1)?;
                    if o.insert(k, v).is_some() {
                        return Err("duplicate".into());
                    }
                    self.ws();
                    let c = *self.b.get(self.at).ok_or("object EOF")?;
                    self.at += 1;
                    if c == b'}' {
                        break;
                    }
                    if c != b',' {
                        return Err("object separator".into());
                    }
                }
                Ok(J::O(o))
            }
            b'0'..=b'9' => {
                let start = self.at - 1;
                while self.b.get(self.at).is_some_and(u8::is_ascii_digit) {
                    self.at += 1;
                }
                if self.at - start > 1 && c == b'0' {
                    return Err("leading zero".into());
                }
                Ok(J::N(
                    std::str::from_utf8(&self.b[start..self.at])
                        .map_err(|_| "number")?
                        .parse()
                        .map_err(|_| "integer overflow")?,
                ))
            }
            _ => Err("JSON subset".into()),
        }
    }
}
fn parse(b: &[u8]) -> R<J> {
    if b.len() > 1048576 {
        return Err("header bound".into());
    }
    let mut p = Parser { b, at: 0, nodes: 0 };
    let v = p.value(0)?;
    p.ws();
    if p.at != b.len() {
        return Err("trailing JSON".into());
    }
    Ok(v)
}
#[derive(Clone, Copy)]
struct D {
    h: usize,
    i: usize,
    q: usize,
    k: usize,
    w: usize,
    l: usize,
    t: usize,
}
impl D {
    fn from(c: [usize; 7]) -> Self {
        Self {
            h: c[0],
            i: c[1],
            q: c[2],
            k: c[3],
            w: c[4],
            l: c[5],
            t: c[6],
        }
    }
}
struct Real {
    bytes: Vec<u8>,
    tensors: BTreeMap<String, (usize, usize)>,
    start: usize,
}
fn layout() -> BTreeMap<String, Vec<usize>> {
    let mut m = BTreeMap::new();
    m.insert("embed_tokens.weight".into(), vec![151669, 1024]);
    m.insert("norm.weight".into(), vec![1024]);
    for l in 0..28 {
        for (n, s) in [
            ("input_layernorm.weight", vec![1024]),
            ("post_attention_layernorm.weight", vec![1024]),
            ("self_attn.q_proj.weight", vec![2048, 1024]),
            ("self_attn.k_proj.weight", vec![1024, 1024]),
            ("self_attn.v_proj.weight", vec![1024, 1024]),
            ("self_attn.o_proj.weight", vec![1024, 2048]),
            ("self_attn.q_norm.weight", vec![128]),
            ("self_attn.k_norm.weight", vec![128]),
            ("mlp.gate_proj.weight", vec![3072, 1024]),
            ("mlp.up_proj.weight", vec![3072, 1024]),
            ("mlp.down_proj.weight", vec![1024, 3072]),
        ] {
            m.insert(format!("layers.{l}.{n}"), s);
        }
    }
    m
}
impl Real {
    fn new(bytes: Vec<u8>) -> R<Self> {
        if bytes.len() != 1191586416 {
            return Err("artifact extent".into());
        }
        let n = u64::from_le_bytes(bytes[..8].try_into().map_err(|_| "length")?) as usize;
        if n > 1048576 || n + 8 > bytes.len() {
            return Err("header extent".into());
        }
        let header = parse(&bytes[8..8 + n])?;
        let o = header.obj()?;
        let expected = layout();
        let mut tensors = BTreeMap::new();
        for (name, v) in o {
            if name == "__metadata__" {
                v.obj()?;
                continue;
            }
            let shape = expected.get(name).ok_or("extra tensor")?;
            let fields = v.obj()?;
            if fields.len() != 3 || fields.get("dtype").ok_or("dtype")?.text()? != "BF16" {
                return Err("dtype/fields".into());
            }
            let a = fields.get("shape").ok_or("shape")?.arr()?;
            let got: Vec<usize> = a.iter().map(J::num).collect::<R<_>>()?;
            if &got != shape {
                return Err("shape".into());
            }
            let off = fields.get("data_offsets").ok_or("offsets")?.arr()?;
            if off.len() != 2 {
                return Err("offset count".into());
            }
            let lo = off[0].num()?;
            let hi = off[1].num()?;
            let need = shape
                .iter()
                .try_fold(2usize, |n, x| n.checked_mul(*x))
                .ok_or("extent overflow")?;
            if hi.checked_sub(lo) != Some(need) || hi > bytes.len() - n - 8 {
                return Err("offset extent".into());
            }
            tensors.insert(name.clone(), (lo, hi));
        }
        if tensors.len() != 310 {
            return Err("missing tensor".into());
        }
        let mut spans: Vec<_> = tensors.values().copied().collect();
        spans.sort_unstable();
        let mut end = 0;
        for (lo, hi) in spans {
            if lo != end {
                return Err("overlap/gap".into());
            }
            end = hi;
        }
        if end != bytes.len() - n - 8 {
            return Err("data trailing".into());
        }
        let r = Self {
            bytes,
            tensors,
            start: n + 8,
        };
        for (lo, hi) in r.tensors.values() {
            for p in r.bytes[r.start + lo..r.start + hi].chunks_exact(2) {
                let v = f32::from_bits(u32::from(u16::from_le_bytes([p[0], p[1]])) << 16);
                if !v.is_finite() {
                    return Err("nonfinite BF16".into());
                }
            }
        }
        Ok(r)
    }
    fn slice(&self, name: &str) -> R<&[u8]> {
        let name = name.strip_prefix("model.").ok_or("weight namespace")?;
        let (lo, hi) = self.tensors.get(name).ok_or("missing tensor")?;
        Ok(&self.bytes[self.start + lo..self.start + hi])
    }
}
enum Source<'a> {
    Tiny,
    Real(&'a Real),
}
fn value(i: usize, s: usize) -> f64 {
    (((i * 17 + s * 13) % 31) as f64 - 15.) / 64.
}
fn layername(l: usize, s: &str) -> String {
    format!("model.layers.{l}.{s}")
}
fn at(b: &[u8], i: usize) -> f64 {
    f64::from(f32::from_bits(
        u32::from(u16::from_le_bytes([b[2 * i], b[2 * i + 1]])) << 16,
    ))
}
impl Source<'_> {
    fn mat(&self, name: &str, x: &[f64], rows: usize, salt: usize) -> R<Vec<f64>> {
        let b = if let Self::Real(r) = self {
            Some(r.slice(name)?)
        } else {
            None
        };
        if b.is_some_and(|b| b.len() != 2 * rows * x.len()) {
            return Err("matrix extent".into());
        }
        let mut out = vec![0.; rows];
        for (row, y) in out.iter_mut().enumerate() {
            for (i, a) in x.iter().enumerate() {
                *y += a * if let Some(b) = b {
                    at(b, row * x.len() + i)
                } else {
                    value(row * x.len() + i, salt)
                };
            }
        }
        finite(&out)?;
        Ok(out)
    }
    fn norm(&self, name: &str, x: &[f64], salt: usize) -> R<Vec<f64>> {
        let b = if let Self::Real(r) = self {
            Some(r.slice(name)?)
        } else {
            None
        };
        if b.is_some_and(|b| b.len() != 2 * x.len()) {
            return Err("norm extent".into());
        }
        let rms = (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64 + 1e-6).sqrt();
        let out: Vec<_> = x
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v / rms
                    * if let Some(b) = b {
                        at(b, i)
                    } else {
                        1. + value(i, salt) / 4.
                    }
            })
            .collect();
        finite(&out)?;
        Ok(out)
    }
    fn embedding(&self, token: usize, h: usize) -> R<Vec<f64>> {
        let b = if let Self::Real(r) = self {
            Some(r.slice("model.embed_tokens.weight")?)
        } else {
            None
        };
        Ok((0..h)
            .map(|i| {
                if let Some(b) = b {
                    at(b, token * h + i)
                } else {
                    value(token * h + i, 2)
                }
            })
            .collect())
    }
    fn head(&self, x: &[f64]) -> R<Vec<f64>> {
        if let Self::Real(_) = self {
            normalize(x)
        } else {
            self.mat("unused", x, 7, 2)
        }
    }
}
fn finite(v: &[f64]) -> R<()> {
    if v.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err("nonfinite arithmetic".into())
    }
}
fn rotate(x: &mut [f64], pos: usize) {
    let n = x.len() / 2;
    for j in 0..n {
        let theta = pos as f64 * 1000000f64.powf(-2. * j as f64 / x.len() as f64);
        let (a, b) = (x[j], x[j + n]);
        x[j] = a * theta.cos() - b * theta.sin();
        x[j + n] = b * theta.cos() + a * theta.sin();
    }
}
struct Outcome {
    flat: Vec<f64>,
    head: Vec<f64>,
}
fn execute(d: D, tokens: &[usize], s: Source<'_>) -> R<Outcome> {
    if tokens.len() != d.t
        || d.t == 0
        || d.t > 512
        || d.k == 0
        || !d.q.is_multiple_of(d.k)
        || d.w == 0
        || !d.w.is_multiple_of(2)
    {
        return Err("dimensions".into());
    }
    let mut states = tokens
        .iter()
        .map(|&t| s.embedding(t, d.h))
        .collect::<R<Vec<_>>>()?;
    let mut cache = Vec::new();
    for l in 0..d.l {
        let seed = 11 + l * 19;
        let mut qs = Vec::new();
        let mut ks = Vec::new();
        let mut vs = Vec::new();
        for (t, x) in states.iter().enumerate() {
            let n = s.norm(&layername(l, "input_layernorm.weight"), x, seed)?;
            let mut q = s.mat(
                &layername(l, "self_attn.q_proj.weight"),
                &n,
                d.q * d.w,
                seed + 1,
            )?;
            let mut k = s.mat(
                &layername(l, "self_attn.k_proj.weight"),
                &n,
                d.k * d.w,
                seed + 2,
            )?;
            let v = s.mat(
                &layername(l, "self_attn.v_proj.weight"),
                &n,
                d.k * d.w,
                seed + 3,
            )?;
            for head in q.chunks_mut(d.w) {
                let n = s.norm(&layername(l, "self_attn.q_norm.weight"), head, seed + 4)?;
                head.copy_from_slice(&n);
                rotate(head, t)
            }
            for head in k.chunks_mut(d.w) {
                let n = s.norm(&layername(l, "self_attn.k_norm.weight"), head, seed + 5)?;
                head.copy_from_slice(&n);
                rotate(head, t)
            }
            qs.push(q);
            ks.push(k);
            vs.push(v);
        }
        let mut next = Vec::new();
        for t in 0..d.t {
            let mut attended = vec![0.; d.q * d.w];
            for a in 0..d.q {
                let b = a / (d.q / d.k);
                let mut dots = Vec::new();
                for k in ks.iter().take(t + 1) {
                    dots.push(
                        (0..d.w)
                            .map(|j| qs[t][a * d.w + j] * k[b * d.w + j])
                            .sum::<f64>()
                            / (d.w as f64).sqrt(),
                    )
                }
                let max = dots.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                for v in &mut dots {
                    *v = (*v - max).exp()
                }
                let sum = dots.iter().sum::<f64>();
                for j in 0..d.w {
                    attended[a * d.w + j] =
                        (0..=t).map(|u| dots[u] / sum * vs[u][b * d.w + j]).sum();
                }
            }
            let projected = s.mat(
                &layername(l, "self_attn.o_proj.weight"),
                &attended,
                d.h,
                seed + 6,
            )?;
            let residual: Vec<_> = states[t]
                .iter()
                .zip(projected)
                .map(|(a, b)| a + b)
                .collect();
            let post = s.norm(
                &layername(l, "post_attention_layernorm.weight"),
                &residual,
                seed + 7,
            )?;
            let gate = s.mat(&layername(l, "mlp.gate_proj.weight"), &post, d.i, seed + 8)?;
            let up = s.mat(&layername(l, "mlp.up_proj.weight"), &post, d.i, seed + 9)?;
            let hidden: Vec<_> = gate
                .iter()
                .zip(up)
                .map(|(g, u)| {
                    if *g >= 0. {
                        g / (1. + (-g).exp()) * u
                    } else {
                        let e = g.exp();
                        g * e / (1. + e) * u
                    }
                })
                .collect();
            let down = s.mat(
                &layername(l, "mlp.down_proj.weight"),
                &hidden,
                d.h,
                seed + 10,
            )?;
            let row: Vec<_> = residual.iter().zip(down).map(|(a, b)| a + b).collect();
            finite(&row)?;
            next.push(row);
        }
        states = next;
        for row in ks {
            cache.extend(row)
        }
        for row in vs {
            cache.extend(row)
        }
    }
    let norm = states
        .iter()
        .map(|x| s.norm("model.norm.weight", x, 3))
        .collect::<R<Vec<_>>>()?;
    let logits = norm.iter().map(|x| s.head(x)).collect::<R<Vec<_>>>()?;
    let mut flat: Vec<_> = norm.into_iter().flatten().collect();
    for x in &logits {
        flat.extend_from_slice(x)
    }
    flat.extend(cache);
    let head = logits.last().ok_or("head")?.clone();
    finite(&head)?;
    Ok(Outcome { flat, head })
}
fn self_test() -> R<()> {
    let v = parse(include_bytes!(
        "../fixtures/g57/numerical-reference.v1.json"
    ))?;
    let cases = v.obj()?.get("cases").ok_or("cases")?.arr()?;
    for (i, c) in [
        [3, 7, 4, 2, 4, 2, 3],
        [5, 6, 2, 1, 2, 1, 3],
        [3, 5, 2, 1, 128, 2, 2],
    ]
    .into_iter()
    .enumerate()
    {
        let d = D::from(c);
        let o = execute(d, &(0..d.t).collect::<Vec<_>>(), Source::Tiny)?;
        let expected = cases[i].obj()?.get("values").ok_or("values")?.arr()?;
        if o.flat.len() != expected.len() {
            return Err("reference count".into());
        }
        for (a, b) in o.flat.iter().zip(expected) {
            let b: f64 = b.text()?.parse().map_err(|_| "reference scalar")?;
            if !a.is_finite() || (a - b).abs() > 0.00005 + 0.00005 * b.abs() {
                return Err("reference mismatch".into());
            }
        }
    }
    Ok(())
}
fn normalize(x: &[f64]) -> R<Vec<f64>> {
    finite(x)?;
    let norm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !norm.is_finite() || norm <= 1e-12 { return Err("invalid norm".into()); }
    let result: Vec<_> = x.iter().map(|v| v / norm).collect();
    finite(&result)?;
    Ok(result)
}
fn embedding_self_test() -> R<()> {
    // Literal G67 Decimal80 cases: [3,4] -> [.6,.8]; negative and irrational norm.
    for (input, want) in [
        (vec![3.,4.], vec![0.6,0.8]),
        (vec![-3.,4.], vec![-0.6,0.8]),
        (vec![1.,1.], vec![0.707106781186547524400844,0.707106781186547524400844]),
    ] {
        let got=normalize(&input)?;
        for (a,b) in got.iter().zip(want) { if (a-b).abs()>1e-14 { return Err("Decimal80 normalization".into()); } }
    }
    for input in [vec![0.,0.],vec![1e-13,0.],vec![f64::NAN,1.]] {
        if normalize(&input).is_ok() { return Err("invalid norm admitted".into()); }
    }
    Ok(())
}
fn main() -> R<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len()==2 && args[1]=="--self-test" {
        self_test()?; embedding_self_test()?;
        println!("PASS: independent Decimal80 decoder and G67 normalization cases");
        return Ok(());
    }
    if args.len()!=6 || args[1]!="--weights-stdin" || args[2]!="--tokens" || args[4]!="--output" { return Err("arguments".into()); }
    let data=std::fs::read(&args[3]).map_err(|_|"tokens IO")?;
    if data.len()>16384 || data.get(..8)!=Some(b"G68TOK01") { return Err("token format".into()); }
    let mut at=8usize;
    let read_u32=|at: &mut usize| -> R<usize> {
        let b=data.get(*at..*at+4).ok_or("token extent")?;
        *at+=4; Ok(u32::from_le_bytes(b.try_into().map_err(|_|"token integer")?) as usize)
    };
    if read_u32(&mut at)?!=9 { return Err("case count".into()); }
    let mut cases=Vec::new();
    for i in 0..9 {
        let role=*data.get(at).ok_or("role")?; at+=1;
        if role!=u8::from(i==8 || i%2==1) { return Err("role order".into()); }
        let n=read_u32(&mut at)?;
        if n==0 || n>if i==8 {512} else {32} { return Err("token bound".into()); }
        let mut row=Vec::new();
        for _ in 0..n { let v=read_u32(&mut at)?; if v>=151669 { return Err("token ID".into()); } row.push(v); }
        cases.push(row);
    }
    if at!=data.len() { return Err("token trailing bytes".into()); }
    let mut bytes=Vec::new(); bytes.try_reserve_exact(1191586417).map_err(|_|"allocation")?;
    std::io::stdin().take(1191586417).read_to_end(&mut bytes).map_err(|_|"input IO")?;
    let real=Real::new(bytes)?;
    let mut output=Vec::from(b"G68F6401".as_slice());
    output.extend_from_slice(&9u32.to_le_bytes());
    for tokens in cases {
        let d=D {h:1024,i:3072,q:16,k:8,w:128,l:28,t:tokens.len()};
        let o=execute(d,&tokens,Source::Real(&real))?;
        if o.head.len()!=1024 { return Err("output dimensions".into()); }
        output.extend_from_slice(&(tokens.len() as u32).to_le_bytes());
        output.extend(o.head.iter().flat_map(|v|v.to_le_bytes()));
    }
    use std::io::Write;
    let mut options=std::fs::OpenOptions::new(); options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut f=options.open(&args[5]).map_err(|_|"output create")?;
    f.write_all(&output).map_err(|_|"output IO")?;
    f.sync_all().map_err(|_|"output sync")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn frozen_decimal_cases() {
        super::self_test().unwrap();
        super::embedding_self_test().unwrap();
    }
    #[test]
    fn header_rejects_duplicate_trailing_and_bounds() {
        for b in [
            b"{\"a\":1,\"a\":2}".as_slice(),
            b"{} trailing",
            b"{\"a\":01}",
        ] {
            assert!(super::parse(b).is_err());
        }
        assert!(super::parse(&vec![b' '; 1048577]).is_err());
        assert!(super::Real::new(vec![0; 10]).is_err());
    }
}
