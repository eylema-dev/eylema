//! Trusted provider broker. Constructors and observations are not authentication.
mod codex;
mod json;
pub use codex::*;

use crate::{Artifact, Error, Result, files};
use json::Json;
use std::collections::BTreeSet;

#[derive(Debug)]
pub struct Proposal {
    pub summary: String,
    pub changes: Vec<Artifact>,
}
fn proposal(data: &[u8], scope: &[String]) -> Result<Proposal> {
    if data.len() > 65536 {
        return Err(Error::InvalidInput);
    }
    let value = json::parse(data).map_err(|_| Error::InvalidInput)?;
    let Json::Object(map) = &value else {
        return Err(Error::InvalidInput);
    };
    if map.len() != 2 {
        return Err(Error::InvalidInput);
    }
    let summary = value
        .field("summary")
        .and_then(Json::text)
        .ok_or(Error::InvalidInput)?;
    if summary.is_empty() || summary.len() > 4096 {
        return Err(Error::InvalidInput);
    }
    let Some(Json::Array(list)) = value.field("changes") else {
        return Err(Error::InvalidInput);
    };
    if list.is_empty() || list.len() > 16 {
        return Err(Error::InvalidInput);
    }
    let mut changes = Vec::new();
    let mut paths = BTreeSet::new();
    let mut total = 0;
    for change in list {
        let Json::Object(fields) = change else {
            return Err(Error::InvalidInput);
        };
        if fields.len() != 3 {
            return Err(Error::InvalidInput);
        }
        let path = change
            .field("path")
            .and_then(Json::text)
            .ok_or(Error::InvalidInput)?;
        let text = change
            .field("text")
            .and_then(Json::text)
            .ok_or(Error::InvalidInput)?;
        let Some(Json::Bool(deleted)) = change.field("deleted") else {
            return Err(Error::InvalidInput);
        };
        if path.len() > 256 || !files::permits(scope, path) || !paths.insert(path) {
            return Err(Error::Scope);
        }
        total += text.len();
        if text.len() > 32768 || total > 65536 || *deleted && !text.is_empty() {
            return Err(Error::InvalidInput);
        }
        changes.push(Artifact {
            path: path.into(),
            text: text.into(),
            deleted: *deleted,
        });
    }
    Ok(Proposal {
        summary: summary.into(),
        changes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proposal_paths_duplicate_fields_and_deletion_semantics_fail_closed() {
        let scope = vec!["allowed.txt".into(), "generated/".into()];
        let good = br#"{"summary":"bounded","changes":[{"path":"allowed.txt","text":"","deleted":false}]}"#;
        assert!(!proposal(good, &scope).unwrap().changes[0].deleted);
        for path in [
            "../allowed.txt",
            "/allowed.txt",
            ".git",
            "outside.txt",
            "generated/../x",
            "generated//x",
        ] {
            let text = format!(
                r#"{{"summary":"x","changes":[{{"path":"{path}","text":"x","deleted":false}}]}}"#
            );
            assert!(proposal(text.as_bytes(), &scope).is_err(), "{path}");
        }
        for data in [
            r#"{"summary":"x","changes":[]}"#,
            r#"{"summary":"x","changes":[{"path":"allowed.txt","text":"x","deleted":true}]}"#,
            r#"{"summary":"x","changes":[{"path":"allowed.txt","text":"","deleted":false},{"path":"allowed.txt","text":"","deleted":false}]}"#,
            r#"{"summary":"x","changes":[{"path":"allowed.txt","text":"","deleted":false,"command":"merge"}]}"#,
            r#"{"summary":"x","changes":[{"path":"allowed.txt","text":"","deleted":false}],"authority":true}"#,
        ] {
            assert!(proposal(data.as_bytes(), &scope).is_err());
        }
    }
}
