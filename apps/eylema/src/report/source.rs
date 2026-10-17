use super::types::*;
pub fn parse_document(bytes: &[u8]) -> Result<SourceDocument> {
    if bytes.len() > 1024 {
        return Err(ReportError::Limit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ReportError::InvalidInput)?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() || text.chars().any(char::is_control) {
        return Err(ReportError::InvalidInput);
    }
    let mut language = None;
    let mut site = None;
    let mut owner = None;
    let mut checklist = None;
    for pair in text.split(';') {
        let (key, value) = pair.split_once('=').ok_or(ReportError::InvalidInput)?;
        if value.is_empty() || value.len() > 256 || value.contains('=') {
            return Err(ReportError::InvalidInput);
        }
        let target = match key {
            "language" => &mut language,
            "site" => &mut site,
            "owner" => &mut owner,
            "checklist" => &mut checklist,
            _ => return Err(ReportError::InvalidInput),
        };
        if target.is_some() {
            return Err(ReportError::InvalidInput);
        }
        *target = Some(value.to_owned());
    }
    let language = language.ok_or(ReportError::InvalidInput)?;
    let site = site.ok_or(ReportError::InvalidInput)?;
    if !matches!(language.as_str(), "en" | "tr")
        || site.len() > 128
        || (owner.is_none() && checklist.is_none())
    {
        return Err(ReportError::InvalidInput);
    }
    Ok(SourceDocument {
        language,
        site,
        owner,
        checklist,
    })
}
