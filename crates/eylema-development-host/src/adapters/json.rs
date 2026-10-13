//! Bounded JSON syntax for provider observations; not a replacement schema engine.
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) enum Json {
    Null,
    Bool(bool),
    Number(String),
    Text(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}
impl Json {
    pub(super) fn field(&self, key: &str) -> Option<&Self> {
        if let Self::Object(map) = self {
            map.get(key)
        } else {
            None
        }
    }
    pub(super) fn text(&self) -> Option<&str> {
        if let Self::Text(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub(super) fn unsigned(&self) -> Option<u64> {
        if let Self::Number(s) = self {
            s.parse().ok()
        } else {
            None
        }
    }
}
pub(super) fn parse(input: &[u8]) -> Result<Json, ()> {
    if input.is_empty() || input.len() > 1024 * 1024 || std::str::from_utf8(input).is_err() {
        return Err(());
    }
    let mut reader = Reader {
        input,
        at: 0,
        nodes: 0,
    };
    let value = reader.value(0)?;
    reader.space();
    if reader.at != input.len() {
        return Err(());
    }
    Ok(value)
}
struct Reader<'a> {
    input: &'a [u8],
    at: usize,
    nodes: usize,
}
impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.input.get(self.at).copied()
    }
    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.at += 1;
        }
    }
    fn byte(&mut self, expected: u8) -> Result<(), ()> {
        if self.peek() != Some(expected) {
            return Err(());
        }
        self.at += 1;
        Ok(())
    }
    fn keyword(&mut self, word: &[u8], value: Json) -> Result<Json, ()> {
        if self.input.get(self.at..self.at + word.len()) != Some(word) {
            return Err(());
        }
        self.at += word.len();
        Ok(value)
    }
    fn value(&mut self, depth: usize) -> Result<Json, ()> {
        self.nodes += 1;
        if depth > 16 || self.nodes > 4096 {
            return Err(());
        }
        self.space();
        match self.peek() {
            Some(b'n') => self.keyword(b"null", Json::Null),
            Some(b't') => self.keyword(b"true", Json::Bool(true)),
            Some(b'f') => self.keyword(b"false", Json::Bool(false)),
            Some(b'"') => Ok(Json::Text(self.string()?)),
            Some(b'[') => {
                self.at += 1;
                self.space();
                let mut values = Vec::new();
                if self.peek() == Some(b']') {
                    self.at += 1;
                    return Ok(Json::Array(values));
                }
                loop {
                    values.push(self.value(depth + 1)?);
                    self.space();
                    if self.peek() == Some(b']') {
                        self.at += 1;
                        break;
                    }
                    self.byte(b',')?;
                }
                Ok(Json::Array(values))
            }
            Some(b'{') => {
                self.at += 1;
                self.space();
                let mut map = BTreeMap::new();
                if self.peek() == Some(b'}') {
                    self.at += 1;
                    return Ok(Json::Object(map));
                }
                loop {
                    self.space();
                    let key = self.string()?;
                    self.space();
                    self.byte(b':')?;
                    if map.insert(key, self.value(depth + 1)?).is_some() {
                        return Err(());
                    }
                    self.space();
                    if self.peek() == Some(b'}') {
                        self.at += 1;
                        break;
                    }
                    self.byte(b',')?;
                }
                Ok(Json::Object(map))
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(()),
        }
    }
    fn digits(&mut self) -> Result<(), ()> {
        let begin = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        if self.at == begin { Err(()) } else { Ok(()) }
    }
    fn number(&mut self) -> Result<Json, ()> {
        let begin = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        if self.peek() == Some(b'0') {
            self.at += 1;
        } else {
            self.digits()?;
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            self.digits()?;
        }
        Ok(Json::Number(
            std::str::from_utf8(&self.input[begin..self.at])
                .map_err(|_| ())?
                .into(),
        ))
    }
    fn hex(&mut self) -> Result<u32, ()> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = match self.peek() {
                Some(c @ b'0'..=b'9') => c - b'0',
                Some(c @ b'a'..=b'f') => c - b'a' + 10,
                Some(c @ b'A'..=b'F') => c - b'A' + 10,
                _ => return Err(()),
            };
            self.at += 1;
            value = value * 16 + u32::from(digit);
        }
        Ok(value)
    }
    fn string(&mut self) -> Result<String, ()> {
        self.byte(b'"')?;
        let mut data = Vec::new();
        loop {
            let c = self.peek().ok_or(())?;
            self.at += 1;
            match c {
                b'"' => break,
                0..=31 => return Err(()),
                b'\\' => {
                    let escape = self.peek().ok_or(())?;
                    self.at += 1;
                    match escape {
                        b'"' | b'\\' | b'/' => data.push(escape),
                        b'b' => data.push(8),
                        b'f' => data.push(12),
                        b'n' => data.push(b'\n'),
                        b'r' => data.push(b'\r'),
                        b't' => data.push(b'\t'),
                        b'u' => {
                            let mut scalar = self.hex()?;
                            if (0xd800..=0xdbff).contains(&scalar) {
                                self.byte(b'\\')?;
                                self.byte(b'u')?;
                                let low = self.hex()?;
                                if !(0xdc00..=0xdfff).contains(&low) {
                                    return Err(());
                                }
                                scalar = 0x10000 + ((scalar - 0xd800) << 10) + low - 0xdc00;
                            }
                            let value = char::from_u32(scalar).ok_or(())?;
                            let mut buffer = [0; 4];
                            data.extend_from_slice(value.encode_utf8(&mut buffer).as_bytes());
                        }
                        _ => return Err(()),
                    }
                }
                _ => data.push(c),
            }
            if data.len() > 65536 {
                return Err(());
            }
        }
        String::from_utf8(data).map_err(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_and_escape_sequences_are_decoded_without_losing_meaning() {
        let value = parse(br#"{"text":"T\u00fcrk\u00e7e \ud83d\ude80\n\"\\\/"}"#).unwrap();
        assert_eq!(
            value.field("text").unwrap().text(),
            Some("Türkçe 🚀\n\"\\/")
        );
    }
    #[test]
    fn invalid_json_duplicate_keys_surrogates_numbers_and_trailing_data_fail() {
        for data in [
            r#"{"a":1,"a":2}"#,
            r#"["\ud800"]"#,
            r#"["\udc00"]"#,
            r#"["\ud800\u0041"]"#,
            r#""\q""#,
            "[1,]",
            r#"{"a":1,}"#,
            "01",
            "1.",
            "1e",
            "--1",
            "true false",
            "\"\n\"",
            "[",
            "",
        ] {
            assert!(parse(data.as_bytes()).is_err(), "{data}");
        }
        assert!(parse(&[0xff]).is_err());
    }
    #[test]
    fn depth_node_input_and_string_limits_are_enforced() {
        assert!(parse(format!("{}0{}", "[".repeat(17), "]".repeat(17)).as_bytes()).is_err());
        assert!(parse(format!("[{}0]", "0,".repeat(4096)).as_bytes()).is_err());
        assert!(parse(&vec![b' '; 1024 * 1024 + 1]).is_err());
        assert!(parse(format!("\"{}\"", "x".repeat(65537)).as_bytes()).is_err());
    }
    #[test]
    fn unsigned_observations_never_accept_negative_fractional_or_overflow_numbers() {
        assert_eq!(parse(b"42").unwrap().unsigned(), Some(42));
        for data in ["-1", "1.0", "1e2", "18446744073709551616"] {
            assert_eq!(parse(data.as_bytes()).unwrap().unsigned(), None);
        }
    }
}
