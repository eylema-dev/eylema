use super::{
    CpuBudget, CpuControl, CpuError as E,
    attention::{self, Attention},
    budget::Meter,
    ops,
    profile::Dimensions,
    weights::BoundWeights,
};
pub(crate) struct Workspace {
    pub x: Vec<f32>,
    pub normalized: Vec<f32>,
    pub q: Vec<f32>,
    pub k: Vec<f32>,
    pub v: Vec<f32>,
    pub attended: Vec<f32>,
    pub projected: Vec<f32>,
    pub post: Vec<f32>,
    pub gate: Vec<f32>,
    pub up: Vec<f32>,
    pub hidden: Vec<f32>,
    pub down: Vec<f32>,
    pub final_norm: Vec<f32>,
    pub logits: Vec<f32>,
    pub keys: Vec<f32>,
    pub values: Vec<f32>,
    pub scores: Vec<f32>,
    pub layers: Vec<f32>,
}
impl Workspace {
    fn new(d: Dimensions, m: &mut Meter, decision: bool, embedding: bool) -> Result<Self, E> {
        d.validate()?;
        let cache = d.cache(m.budget.context)?;
        // Reserve all workspace plus every possible escaped logit result before any
        // allocation. Returned logits can outlive the session; they are never refunded.
        let logits_len = if decision {
            2
        } else if embedding {
            0
        } else {
            d.vocab
        };
        let lengths = [
            d.h,
            d.h,
            d.qh * d.w,
            d.kh * d.w,
            d.kh * d.w,
            d.qh * d.w,
            d.h,
            d.h,
            d.inter,
            d.inter,
            d.inter,
            d.h,
            d.h,
            logits_len,
            cache,
            cache,
            m.budget.context,
            d.layers * d.h,
        ];
        let mut total = 0u64;
        for n in lengths {
            total = total.checked_add(n as u64).ok_or(E::ResidentLimit)?;
        }
        let outputs = (if decision || embedding { 0 } else { d.vocab } as u64)
            .checked_mul(m.budget.context as u64)
            .ok_or(E::ResidentLimit)?;
        let known = total
            .checked_add(outputs)
            .and_then(|v| v.checked_mul(4))
            .and_then(|v| v.checked_add(m.resident))
            .ok_or(E::ResidentLimit)?;
        if known > m.budget.max_resident_bytes {
            return Err(E::ResidentLimit);
        }
        m.preflight(total)?;
        Ok(Self {
            x: m.zeros(d.h)?,
            normalized: m.zeros(d.h)?,
            q: m.zeros(d.qh * d.w)?,
            k: m.zeros(d.kh * d.w)?,
            v: m.zeros(d.kh * d.w)?,
            attended: m.zeros(d.qh * d.w)?,
            projected: m.zeros(d.h)?,
            post: m.zeros(d.h)?,
            gate: m.zeros(d.inter)?,
            up: m.zeros(d.inter)?,
            hidden: m.zeros(d.inter)?,
            down: m.zeros(d.h)?,
            final_norm: m.zeros(d.h)?,
            logits: m.zeros(logits_len)?,
            keys: m.zeros(cache)?,
            values: m.zeros(cache)?,
            scores: m.zeros(m.budget.context)?,
            layers: m.zeros(d.layers * d.h)?,
        })
    }
}
pub(crate) struct Engine<'a> {
    pub d: Dimensions,
    pub weights: BoundWeights<'a>,
    pub meter: Meter,
    pub ws: Workspace,
    pub pos: usize,
    pub valid: bool,
    selected: Option<[usize; 2]>,
    embedding: bool,
    started: bool,
}
impl<'a> Engine<'a> {
    pub fn new(
        d: Dimensions,
        weights: BoundWeights<'a>,
        budget: CpuBudget,
        control: CpuControl,
        base_bytes: u64,
    ) -> Result<Self, E> {
        d.validate()?;
        let mut meter = Meter::new(budget, control, base_bytes)?;
        let ws = Workspace::new(d, &mut meter, false, false)?;
        Ok(Self {
            d,
            weights,
            meter,
            ws,
            pos: 0,
            valid: true,
            selected: None,
            embedding: false,
            started: false,
        })
    }
    pub(crate) fn new_decision(
        d: Dimensions,
        weights: BoundWeights<'a>,
        budget: crate::decision::DecisionBudget,
        control: CpuControl,
        base: u64,
        rows: [usize; 2],
    ) -> Result<Self, E> {
        d.validate()?;
        if rows[0] >= d.vocab || rows[1] >= d.vocab || rows[0] == rows[1] {
            return Err(E::InvalidLayout);
        }
        let mut meter = Meter::decision(budget, control, base)?;
        let ws = Workspace::new(d, &mut meter, true, false)?;
        Ok(Self {
            d,
            weights,
            meter,
            ws,
            pos: 0,
            valid: true,
            started: false,
            selected: Some(rows),
            embedding: false,
        })
    }
    pub(crate) fn new_embedding(
        d: Dimensions,
        weights: BoundWeights<'a>,
        context: usize,
        max_resident_bytes: u64,
        max_work: u64,
        control: CpuControl,
        base_bytes: u64,
    ) -> Result<Self, E> {
        d.validate()?;
        let mut meter =
            Meter::embedding(context, max_resident_bytes, max_work, control, base_bytes)?;
        let ws = Workspace::new(d, &mut meter, false, true)?;
        Ok(Self {
            d,
            weights,
            meter,
            ws,
            pos: 0,
            valid: true,
            selected: None,
            embedding: true,
            started: false,
        })
    }
    pub(crate) fn prefill_decision(&mut self, tokens: &[u32]) -> Result<(), E> {
        let result = (|| {
            if !self.valid || self.started || self.selected.is_none() {
                return Err(E::InvalidState);
            }
            if tokens.is_empty()
                || tokens.len() > self.meter.budget.context
                || tokens.iter().any(|&v| v as usize >= self.d.vocab)
            {
                return Err(E::ContextLimit);
            }
            self.started = true;
            for (i, &t) in tokens.iter().enumerate() {
                self.compute_projection(t, i + 1 == tokens.len())?;
            }
            Ok(())
        })();
        self.poison(result)
    }
    pub(crate) fn prefill_embedding(&mut self, tokens: &[u32]) -> Result<(), E> {
        let result = (|| {
            if !self.embedding || !self.valid || self.started || self.pos != 0 {
                return Err(E::InvalidState);
            }
            if tokens.is_empty()
                || tokens.len() > self.meter.budget.context
                || tokens.iter().any(|&v| v as usize >= self.d.vocab)
            {
                return Err(E::ContextLimit);
            }
            let required = super::profile::embedding_work_bound(self.d, tokens.len())?;
            self.meter.preflight(required)?;
            self.started = true;
            for (i, &token) in tokens.iter().enumerate() {
                self.compute_projection(token, i + 1 == tokens.len())?;
            }
            Ok(())
        })();
        self.poison(result)
    }

    pub(crate) fn normalized_embedding(&mut self) -> Result<Vec<f32>, E> {
        let result = (|| {
            if !self.embedding || !self.valid || self.pos == 0 {
                return Err(E::InvalidState);
            }
            self.meter.control.check()?;
            let values = &self.ws.final_norm;
            if values.len() != self.d.h {
                return Err(E::InvalidLayout);
            }
            let mut output = self.meter.zeros(values.len())?;
            ops::l2_normalize(values, &mut output, &mut self.meter)?;
            self.meter.control.check()?;
            Ok(std::mem::take(&mut output))
        })();
        self.poison(result)
    }
    fn poison<T>(&mut self, result: Result<T, E>) -> Result<T, E> {
        if result.is_err() {
            self.valid = false;
        }
        result
    }
    pub fn prefill_ids(&mut self, tokens: &[u32]) -> Result<(), E> {
        let result = (|| {
            if !self.valid || self.started || self.pos != 0 {
                return Err(E::InvalidState);
            }
            if tokens.is_empty() {
                return Err(E::InvalidInput);
            }
            if tokens
                .len()
                .checked_add(self.meter.budget.max_new_tokens)
                .is_none_or(|v| v > self.meter.budget.context)
            {
                return Err(E::ContextLimit);
            }
            if tokens.iter().any(|&t| t as usize >= self.d.vocab) {
                return Err(E::InvalidInput);
            }
            self.started = true;
            for &t in tokens {
                self.step(t)?;
            }
            Ok(())
        })();
        self.poison(result)
    }
    pub fn decode_id(&mut self, token: u32) -> Result<(), E> {
        let result = if !self.valid || !self.started {
            Err(E::InvalidState)
        } else if token == 151645 {
            Err(E::InvalidInput)
        } else {
            self.step(token)
        };
        self.poison(result)
    }
    pub fn step(&mut self, token: u32) -> Result<(), E> {
        let result = self.compute(token);
        self.poison(result)
    }
    fn compute(&mut self, token: u32) -> Result<(), E> {
        self.compute_projection(token, true)
    }
    fn compute_projection(&mut self, token: u32, project: bool) -> Result<(), E> {
        if !self.valid {
            return Err(E::InvalidState);
        }
        self.meter.control.check()?;
        if token as usize >= self.d.vocab {
            return Err(E::InvalidInput);
        }
        if self.pos >= self.meter.budget.context {
            return Err(E::ContextLimit);
        }
        let d = self.d;
        let w = &self.weights;
        let x = &mut self.ws;
        let m = &mut self.meter;
        // Token work is preflighted before reads; this lower bound includes all
        // matrix conversions/products/adds. Every actual operation is then charged.
        let matrix_values = d.layers as u64
            * ((d.qh * d.w * d.h + 2 * d.kh * d.w * d.h + d.h * d.qh * d.w + 3 * d.inter * d.h)
                as u64)
            + (if project {
                if self.selected.is_some() {
                    2 * d.h
                } else {
                    d.vocab * d.h
                }
            } else {
                0
            }) as u64;
        m.preflight(matrix_values.checked_mul(3).ok_or(E::WorkLimit)?)?;
        m.charge(d.h as u64)?;
        for (i, v) in x.x.iter_mut().enumerate() {
            *v = w.embedding.at(token as usize * d.h + i);
        }
        for l in 0..d.layers {
            m.control.check()?;
            let layer = w.layers[l].ok_or(E::InvalidLayout)?;
            ops::norm(&x.x, layer.input, &mut x.normalized, m)?;
            ops::matvec(layer.q, &x.normalized, &mut x.q, m)?;
            ops::matvec(layer.k, &x.normalized, &mut x.k, m)?;
            ops::matvec(layer.v, &x.normalized, &mut x.v, m)?;
            for head in x.q.chunks_mut(d.w) {
                ops::norm_head(head, layer.qnorm, m)?;
                ops::rope(head, self.pos, m)?;
            }
            for head in x.k.chunks_mut(d.w) {
                ops::norm_head(head, layer.knorm, m)?;
                ops::rope(head, self.pos, m)?;
            }
            let kw = d.kh * d.w;
            let start = l * m.budget.context * kw;
            let offset = start + self.pos * kw;
            m.charge((2 * kw) as u64)?;
            x.keys[offset..offset + kw].copy_from_slice(&x.k);
            x.values[offset..offset + kw].copy_from_slice(&x.v);
            attention::apply(
                d,
                self.pos,
                Attention {
                    q: &x.q,
                    keys: &x.keys[start..start + m.budget.context * kw],
                    values: &x.values[start..start + m.budget.context * kw],
                    scores: &mut x.scores,
                    out: &mut x.attended,
                },
                m,
            )?;
            ops::matvec(layer.o, &x.attended, &mut x.projected, m)?;
            ops::add(&mut x.x, &x.projected, m)?;
            ops::norm(&x.x, layer.post, &mut x.post, m)?;
            ops::matvec(layer.gate, &x.post, &mut x.gate, m)?;
            ops::matvec(layer.up, &x.post, &mut x.up, m)?;
            for (block, ys) in x.hidden.chunks_mut(512).enumerate() {
                m.charge((ys.len() * 7) as u64)?;
                for (j, y) in ys.iter_mut().enumerate() {
                    let i = block * 512 + j;
                    let g = x.gate[i];
                    let silu = if g >= 0. {
                        g / (1. + (-g).exp())
                    } else {
                        let e = g.exp();
                        g * e / (1. + e)
                    };
                    *y = ops::finite(silu * x.up[i])?;
                }
            }
            ops::matvec(layer.down, &x.hidden, &mut x.down, m)?;
            ops::add(&mut x.x, &x.down, m)?;
            m.charge(d.h as u64)?;
            x.layers[l * d.h..(l + 1) * d.h].copy_from_slice(&x.x);
        }
        if project {
            ops::norm(&x.x, w.norm, &mut x.final_norm, m)?;
            if let Some(rows) = self.selected {
                m.preflight((d.h as u64) * 6 + 2)?;
                for (j, row) in rows.into_iter().enumerate() {
                    let mut sum = 0.;
                    for (block, inputs) in x.final_norm.chunks(1024).enumerate() {
                        m.charge((inputs.len() * 3) as u64)?;
                        for (i, v) in inputs.iter().enumerate() {
                            sum += w.embedding.at(row * d.h + block * 1024 + i) * v;
                        }
                    }
                    m.charge(1)?;
                    x.logits[j] = ops::finite(sum)?;
                }
            } else if !self.embedding {
                ops::matvec(w.embedding, &x.final_norm, &mut x.logits, m)?;
            }
        }
        m.control.check()?;
        self.pos += 1;
        Ok(())
    }
    pub fn output(&mut self) -> Result<super::CpuLogits, E> {
        let result = (|| {
            if self.embedding {
                return Err(E::UnsupportedProfile);
            }
            if !self.valid || self.pos == 0 {
                return Err(E::InvalidState);
            }
            let mut values = self.meter.zeros(self.d.vocab)?;
            for (xs, ys) in self.ws.logits.chunks(1024).zip(values.chunks_mut(1024)) {
                self.meter.charge(xs.len() as u64)?;
                ys.copy_from_slice(xs);
            }
            self.meter.control.check()?;
            Ok(super::CpuLogits { values })
        })();
        self.poison(result)
    }
}
