use super::*;
use eylema_contracts::skill::{OWNED_SKILL_ADAPTER, SkillDefinition};
fn skill_body(bytes: &[u8]) -> Result<(), RouteError> {
    // Closed bounded preflight before the general decoder can allocate containers.
    fn value(b: &[u8], i: &mut usize, d: usize, n: &mut usize) -> Result<(), RouteError> {
        if d > 8 || *n >= 256 {
            return Err(RouteError::Limit);
        }
        *n += 1;
        let tag = *b.get(*i).ok_or(RouteError::InvalidInput)?;
        *i += 1;
        match tag {
            0..=2 => {}
            3 | 4 => *i = i.checked_add(8).ok_or(RouteError::Limit)?,
            5..=7 => {
                let end = i.checked_add(4).ok_or(RouteError::Limit)?;
                let len = u32::from_be_bytes(
                    b.get(*i..end)
                        .ok_or(RouteError::InvalidInput)?
                        .try_into()
                        .map_err(|_| RouteError::InvalidInput)?,
                ) as usize;
                *i = end;
                if tag == 7 {
                    if len > 16 {
                        return Err(RouteError::Limit);
                    }
                    for _ in 0..len {
                        value(b, i, d + 1, n)?;
                    }
                } else {
                    *i = i.checked_add(len).ok_or(RouteError::Limit)?;
                }
            }
            _ => return Err(RouteError::InvalidInput),
        }
        if *i > b.len() {
            return Err(RouteError::InvalidInput);
        }
        Ok(())
    }
    if bytes.len() > 4096 || !bytes.starts_with(codec::HEADER) {
        return Err(RouteError::Limit);
    }
    let mut i = codec::HEADER.len();
    value(bytes, &mut i, 0, &mut 0)?;
    if i != bytes.len() {
        return Err(RouteError::InvalidInput);
    }
    let v = codec::decode(bytes).map_err(|_| RouteError::InvalidInput)?;
    if codec::encode(&v).map_err(|_| RouteError::InvalidInput)? != bytes {
        return Err(RouteError::InvalidInput);
    }
    let s = SkillDefinition::from_value(&v).map_err(|_| RouteError::InvalidInput)?;
    if s.model_dependency().is_some() || s.adapter().as_str() != OWNED_SKILL_ADAPTER {
        return Err(RouteError::Unavailable);
    }
    Ok(())
}
impl Router<'_> {
    pub fn catalog<T: Storage, H: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<T, H, C>,
        s: &DataSession,
        requested: &[RouteId],
        c: &mut RouteControl,
    ) -> Result<Catalog, RouteError> {
        self.live(data)?;
        c.charge(16 * 8 * 128)?;
        let entries = self.entries(requested, 16)?;
        let pins: Vec<_> = entries.iter().flat_map(|e| e.sources.clone()).collect();
        let observed = self.observe(data, s, &pins, c)?;
        let value = Value::List(
            entries
                .iter()
                .map(|e| {
                    Value::List(vec![
                        text(e.id.as_str()),
                        number(e.revision.get()),
                        Value::Bool(e.supported()),
                    ])
                })
                .collect(),
        );
        let bytes = encode(&value, 8192, c)?;
        let now = self.recheck(data, s, &observed, c)?;
        if entries.iter().any(|e| !e.quality.fresh(now)) {
            return Err(RouteError::Stale);
        }
        Ok(Catalog {
            bytes,
            entries: entries.len(),
        })
    }
    pub fn describe<T: Storage, H: Sha256, C: DataClock>(
        &self,
        data: &mut ProjectStore<T, H, C>,
        s: &DataSession,
        requested: &[RouteId],
        c: &mut RouteControl,
    ) -> Result<Description, RouteError> {
        self.live(data)?;
        c.charge(8 * 8 * 128)?;
        let entries = self.entries(requested, 8)?;
        let pins: Vec<_> = entries.iter().flat_map(|e| e.sources.clone()).collect();
        let observed = self.observe(data, s, &pins, c)?;
        if observed.len() > 8 {
            return Err(RouteError::Limit);
        }
        let mut total = 0usize;
        for o in &observed {
            c.charge(262144)?;
            let n = data
                .authorized_payload_len(s, &o.pin.reference, DataUse::Execute)
                .map_err(|_| RouteError::Forbidden)?;
            total = total.checked_add(n).ok_or(RouteError::Limit)?;
            if total > 32768 {
                return Err(RouteError::Limit);
            }
        }
        c.reserve(total * 4 + 65536 + 256 * std::mem::size_of::<Value>())?;
        c.charge(total * 4)?;
        let mut bodies = vec![];
        for o in &observed {
            c.check()?;
            let bytes = data
                .read(s, &o.pin.reference, DataUse::Execute)
                .map_err(|_| RouteError::Forbidden)?;
            if bytes.starts_with(codec::HEADER) {
                skill_body(&bytes)?;
            } else {
                std::str::from_utf8(&bytes).map_err(|_| RouteError::InvalidInput)?;
            }
            bodies.push(source_value(o, Value::Bytes(bytes)));
        }
        let value = Value::List(vec![
            text("route-description-v1"),
            text(self.config.registry.pin.id.as_str()),
            number(self.config.registry.pin.revision.get()),
            text(self.config.policy.pin.id.as_str()),
            number(self.config.policy.pin.revision.get()),
            Value::List(entries.iter().map(|e| entry_value(e)).collect()),
            Value::List(bodies),
        ]);
        let bytes = encode(&value, 16384, c)?;
        let now = self.recheck(data, s, &observed, c)?;
        if entries.iter().any(|e| !e.quality.fresh(now)) {
            return Err(RouteError::Stale);
        }
        Ok(Description {
            bytes,
            entries: requested.to_vec(),
            observed,
            identity: self.identity.clone(),
        })
    }
}
