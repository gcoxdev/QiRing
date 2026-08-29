use crate::model::{
    CsvColumnMapping, CsvImportPreview, CustomField, ItemInput, SecurityQuestion, VaultItem, VaultItemType,
};
use crate::validation::validate_item_input;
use crate::CoreError;
use anyhow::Context;
use std::collections::HashSet;

const MAX_CSV_BYTES: usize = 32 * 1024 * 1024;
const MAX_CSV_ROWS: usize = 100_000;
const MAX_CSV_COLUMNS: usize = 128;
const MAX_CSV_CELL_BYTES: usize = 512 * 1024;
// Do not allow repeated headers to expand imported notes beyond the maximum
// size of the CSV source itself.
const MAX_CSV_IMPORT_NOTES_BYTES: usize = MAX_CSV_BYTES;
const FORMAT_VERSION: &str = "1";

const HEADERS: [&str; 12] = [
    "qiring_format_version",
    "item_type",
    "title",
    "username",
    "password",
    "url",
    "notes",
    "tags",
    "category",
    "security_questions",
    "custom_fields",
    "totp_secret",
];

pub fn csv_template_bytes() -> Vec<u8> {
    write_csv_rows([HEADERS.iter().copied()])
}

pub(crate) fn export_csv_bytes(items: &[VaultItem]) -> anyhow::Result<Vec<u8>> {
    let mut sorted = items.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        left.folder
            .cmp(&right.folder)
            .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut rows = Vec::with_capacity(sorted.len() + 1);
    rows.push(HEADERS.iter().map(|value| (*value).to_string()).collect());
    for item in sorted {
        let item_type = match item.item_type {
            VaultItemType::Login => "login",
            VaultItemType::SecureNote => "secure_note",
        };
        rows.push(vec![
            FORMAT_VERSION.to_string(),
            item_type.to_string(),
            item.title.clone(),
            item.username.clone().unwrap_or_default(),
            item.password.clone().unwrap_or_default(),
            item.url.clone().unwrap_or_default(),
            item.notes.clone().unwrap_or_default(),
            serde_json::to_string(&item.tags).context("serialize CSV tags")?,
            item.folder.clone().unwrap_or_default(),
            serde_json::to_string(&item.security_questions).context("serialize CSV security questions")?,
            serde_json::to_string(&item.custom_fields).context("serialize CSV custom fields")?,
            item.totp_secret.clone().unwrap_or_default(),
        ]);
    }
    Ok(write_csv_rows(
        rows.iter().map(|row| row.iter().map(String::as_str)),
    ))
}

pub(crate) fn preview_csv_bytes(payload: &[u8]) -> anyhow::Result<CsvImportPreview> {
    let table = parse_csv(payload)?;
    let headers = table[0].clone();
    let mapping = suggest_mapping(&headers);
    let canonical = HEADERS
        .iter()
        .all(|expected| headers.iter().any(|header| header == expected));
    let mut warnings = Vec::new();
    if mapping.title.is_none() {
        warnings.push("Map a source column to Qi Name before importing.".into());
    }
    if !canonical {
        warnings.push(
            "This is not a QiRing template file. Review every suggested column mapping before importing."
                .into(),
        );
    }
    if mapping.security_questions.is_none() || mapping.custom_fields.is_none() {
        warnings.push(
            "Security questions or custom fields without mapped JSON columns can only be preserved in Notes."
                .into(),
        );
    }
    let sample_rows = table
        .iter()
        .skip(1)
        .take(5)
        .map(|row| row.iter().map(|cell| decode_cell(cell, canonical)).collect())
        .collect();
    Ok(CsvImportPreview {
        headers,
        row_count: table.len() - 1,
        sample_rows,
        canonical,
        suggested_mapping: mapping,
        warnings,
    })
}

pub(crate) fn import_inputs_from_csv(
    payload: &[u8],
    mapping: &CsvColumnMapping,
) -> anyhow::Result<(Vec<ItemInput>, Vec<String>)> {
    let table = parse_csv(payload)?;
    let headers = &table[0];
    let title_index = mapped_index(headers, &mapping.title, "Qi Name")?
        .ok_or_else(|| CoreError::InvalidInput("a Qi Name column mapping is required".into()))?;
    let indices = MappingIndices {
        item_type: mapped_index(headers, &mapping.item_type, "item type")?,
        title: title_index,
        username: mapped_index(headers, &mapping.username, "username")?,
        password: mapped_index(headers, &mapping.password, "password")?,
        url: mapped_index(headers, &mapping.url, "URL")?,
        notes: mapped_index(headers, &mapping.notes, "notes")?,
        tags: mapped_index(headers, &mapping.tags, "tags")?,
        category: mapped_index(headers, &mapping.category, "category")?,
        security_questions: mapped_index(headers, &mapping.security_questions, "security questions")?,
        custom_fields: mapped_index(headers, &mapping.custom_fields, "custom fields")?,
        totp_secret: mapped_index(headers, &mapping.totp_secret, "TOTP secret")?,
    };
    let canonical = HEADERS
        .iter()
        .all(|expected| headers.iter().any(|header| header == expected));
    let mapped = indices.all().into_iter().flatten().collect::<HashSet<_>>();
    let version_index = headers
        .iter()
        .position(|header| header == "qiring_format_version");
    let mut inputs = Vec::with_capacity(table.len() - 1);
    let mut total_notes_bytes = 0usize;

    for (offset, row) in table.iter().skip(1).enumerate() {
        let row_number = offset + 2;
        if canonical && version_index.is_some_and(|index| decode_cell(&row[index], true) != FORMAT_VERSION) {
            return Err(CoreError::InvalidInput(format!(
                "CSV row {row_number}: qiring_format_version must be 1"
            ))
            .into());
        }
        let decode = |index: Option<usize>| {
            index
                .map(|index| decode_cell(&row[index], canonical))
                .unwrap_or_default()
        };
        let item_type = match decode(indices.item_type).trim().to_ascii_lowercase().as_str() {
            "" | "login" | "password" => VaultItemType::Login,
            "secure_note" | "secure note" | "note" => VaultItemType::SecureNote,
            value => {
                return Err(CoreError::InvalidInput(format!(
                    "CSV row {row_number}: item_type '{value}' is invalid; use 'login' or 'secure_note'"
                ))
                .into())
            }
        };
        let mut notes = optional(decode(indices.notes));
        if mapping.include_unmapped_in_notes {
            let extras = headers
                .iter()
                .enumerate()
                .filter(|(index, _)| !mapped.contains(index) && Some(*index) != version_index)
                .filter_map(|(index, header)| {
                    let value = decode_cell(&row[index], canonical);
                    (!value.trim().is_empty()).then(|| format!("{header}: {value}"))
                })
                .collect::<Vec<_>>();
            if !extras.is_empty() {
                let suffix = extras.join("\n");
                notes = Some(match notes {
                    Some(value) if !value.is_empty() => format!("{value}\n\nImported fields:\n{suffix}"),
                    _ => format!("Imported fields:\n{suffix}"),
                });
            }
        }
        let tags = parse_tags(&decode(indices.tags), row_number)?;
        let security_questions = parse_json_array::<SecurityQuestion>(
            &decode(indices.security_questions),
            "security questions",
            row_number,
        )?;
        let custom_fields =
            parse_json_array::<CustomField>(&decode(indices.custom_fields), "custom fields", row_number)?;
        let input = ItemInput {
            item_type,
            title: decode_cell(&row[indices.title], canonical).trim().to_string(),
            username: optional(decode(indices.username)),
            password: optional(decode(indices.password)),
            url: optional(decode(indices.url)),
            notes,
            tags,
            folder: optional(decode(indices.category)),
            icon_data_url: None,
            security_questions,
            custom_fields,
            totp_secret: optional(decode(indices.totp_secret)),
        };
        validate_item_input(&input).map_err(|error| anyhow::anyhow!("CSV row {row_number}: {error}"))?;
        total_notes_bytes = total_notes_bytes
            .checked_add(input.notes.as_ref().map_or(0, String::len))
            .ok_or_else(|| CoreError::InvalidInput("CSV import exceeds the aggregate Notes limit".into()))?;
        if total_notes_bytes > MAX_CSV_IMPORT_NOTES_BYTES {
            return Err(CoreError::InvalidInput(format!(
                "CSV row {row_number}: import exceeds the 32 MiB aggregate Notes limit"
            ))
            .into());
        }
        inputs.push(input);
    }

    let mut warnings = Vec::new();
    if !canonical {
        warnings.push("Imported from a user-mapped CSV file.".into());
    }
    if mapping.include_unmapped_in_notes {
        warnings.push("Non-empty unmapped columns were appended to Notes.".into());
    }
    Ok((inputs, warnings))
}

struct MappingIndices {
    item_type: Option<usize>,
    title: usize,
    username: Option<usize>,
    password: Option<usize>,
    url: Option<usize>,
    notes: Option<usize>,
    tags: Option<usize>,
    category: Option<usize>,
    security_questions: Option<usize>,
    custom_fields: Option<usize>,
    totp_secret: Option<usize>,
}

impl MappingIndices {
    fn all(&self) -> [Option<usize>; 11] {
        [
            self.item_type,
            Some(self.title),
            self.username,
            self.password,
            self.url,
            self.notes,
            self.tags,
            self.category,
            self.security_questions,
            self.custom_fields,
            self.totp_secret,
        ]
    }
}

fn suggest_mapping(headers: &[String]) -> CsvColumnMapping {
    let find = |aliases: &[&str]| {
        headers
            .iter()
            .find(|header| aliases.contains(&normalize_header(header).as_str()))
            .cloned()
    };
    CsvColumnMapping {
        item_type: find(&["item type", "type", "kind"]),
        title: find(&["title", "qi name", "name", "entry name"]),
        username: find(&["username", "user name", "login", "email"]),
        password: find(&["password", "pass", "secret"]),
        url: find(&["url", "website", "web site", "login url"]),
        notes: find(&["notes", "note", "comments"]),
        tags: find(&["tags", "tag", "labels"]),
        category: find(&["category", "folder", "group"]),
        security_questions: find(&["security questions", "questions"]),
        custom_fields: find(&["custom fields", "fields"]),
        totp_secret: find(&["totp secret", "otp secret"]),
        include_unmapped_in_notes: false,
    }
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn mapped_index(headers: &[String], mapping: &Option<String>, label: &str) -> anyhow::Result<Option<usize>> {
    mapping
        .as_ref()
        .map(|header| {
            headers
                .iter()
                .position(|candidate| candidate == header)
                .ok_or_else(|| {
                    CoreError::InvalidInput(format!("mapped {label} column no longer exists")).into()
                })
        })
        .transpose()
}

fn optional(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn parse_tags(value: &str, row: usize) -> anyhow::Result<Vec<String>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        return serde_json::from_str(trimmed).map_err(|_| {
            CoreError::InvalidInput(format!(
                "CSV row {row}: tags must be a JSON string array such as [\"personal\",\"email\"]"
            ))
            .into()
        });
    }
    Ok(trimmed
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect())
}

fn parse_json_array<T: serde::de::DeserializeOwned>(
    value: &str,
    label: &str,
    row: usize,
) -> anyhow::Result<Vec<T>> {
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(value).map_err(|error| {
        CoreError::InvalidInput(format!("CSV row {row}: {label} JSON is invalid: {error}")).into()
    })
}

fn decode_cell(value: &str, canonical: bool) -> String {
    if canonical {
        if let Some(unescaped) = value.strip_prefix('\'') {
            if unescaped
                .as_bytes()
                .first()
                .is_some_and(|byte| matches!(byte, b'=' | b'+' | b'-' | b'@' | b'\t' | b'\r'))
            {
                return unescaped.to_string();
            }
        }
    }
    value.to_string()
}

fn spreadsheet_safe(value: &str) -> String {
    if value
        .as_bytes()
        .first()
        .is_some_and(|byte| matches!(byte, b'=' | b'+' | b'-' | b'@' | b'\t' | b'\r'))
    {
        format!("'{value}")
    } else {
        value.to_string()
    }
}

fn write_csv_rows<'a>(rows: impl IntoIterator<Item = impl IntoIterator<Item = &'a str>>) -> Vec<u8> {
    let mut output = Vec::from(&b"\xef\xbb\xbf"[..]);
    for row in rows {
        let mut first = true;
        for cell in row {
            if !first {
                output.push(b',');
            }
            first = false;
            output.push(b'"');
            for byte in spreadsheet_safe(cell).bytes() {
                if byte == b'"' {
                    output.push(b'"');
                }
                output.push(byte);
            }
            output.push(b'"');
        }
        output.extend_from_slice(b"\r\n");
    }
    output
}

fn parse_csv(payload: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
    if payload.len() > MAX_CSV_BYTES {
        return Err(CoreError::InvalidInput("CSV file exceeds the 32 MiB limit".into()).into());
    }
    let payload = payload.strip_prefix(b"\xef\xbb\xbf").unwrap_or(payload);
    if let Err(error) = std::str::from_utf8(payload) {
        let line = payload[..error.valid_up_to()]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            + 1;
        return Err(CoreError::InvalidInput(format!("CSV line {line}: file must use UTF-8 encoding")).into());
    }

    let mut rows = Vec::<Vec<String>>::new();
    let mut row = Vec::<String>::new();
    let mut field = Vec::<u8>::new();
    let mut quoted = false;
    let mut after_quote = false;
    let mut started = false;
    let mut index = 0;
    let mut line = 1;
    let finish_field = |row: &mut Vec<String>, field: &mut Vec<u8>, line: usize| -> anyhow::Result<()> {
        if field.len() > MAX_CSV_CELL_BYTES {
            return Err(
                CoreError::InvalidInput(format!("CSV line {line}: cell exceeds the 512 KiB limit")).into(),
            );
        }
        row.push(String::from_utf8(std::mem::take(field)).expect("CSV UTF-8 was checked"));
        if row.len() > MAX_CSV_COLUMNS {
            return Err(
                CoreError::InvalidInput(format!("CSV line {line}: row has more than 128 columns")).into(),
            );
        }
        Ok(())
    };
    let finish_row = |rows: &mut Vec<Vec<String>>, row: &mut Vec<String>| -> anyhow::Result<()> {
        rows.push(std::mem::take(row));
        if rows.len() > MAX_CSV_ROWS + 1 {
            return Err(CoreError::InvalidInput("CSV has more than 100,000 data rows".into()).into());
        }
        Ok(())
    };

    while index < payload.len() {
        let byte = payload[index];
        if quoted {
            if byte == b'"' {
                if payload.get(index + 1) == Some(&b'"') {
                    field.push(b'"');
                    index += 2;
                    continue;
                }
                quoted = false;
                after_quote = true;
            } else {
                field.push(byte);
                if byte == b'\n' || (byte == b'\r' && payload.get(index + 1) != Some(&b'\n')) {
                    line += 1;
                }
            }
            index += 1;
            continue;
        }
        if after_quote {
            match byte {
                b',' => finish_field(&mut row, &mut field, line)?,
                b'\n' => {
                    finish_field(&mut row, &mut field, line)?;
                    finish_row(&mut rows, &mut row)?;
                    line += 1;
                }
                b'\r' => {
                    finish_field(&mut row, &mut field, line)?;
                    finish_row(&mut rows, &mut row)?;
                    line += 1;
                    if payload.get(index + 1) == Some(&b'\n') {
                        index += 1;
                    }
                }
                _ => {
                    return Err(CoreError::InvalidInput(format!(
                        "CSV line {line}: characters appear after a closing quote"
                    ))
                    .into())
                }
            }
            after_quote = false;
            started = false;
            index += 1;
            continue;
        }
        match byte {
            b'"' if field.is_empty() && !started => {
                quoted = true;
                started = true;
            }
            b'"' => {
                return Err(CoreError::InvalidInput(format!(
                    "CSV line {line}: quote appears inside an unquoted cell"
                ))
                .into())
            }
            b',' => {
                finish_field(&mut row, &mut field, line)?;
                started = false;
            }
            b'\n' => {
                finish_field(&mut row, &mut field, line)?;
                finish_row(&mut rows, &mut row)?;
                started = false;
                line += 1;
            }
            b'\r' => {
                finish_field(&mut row, &mut field, line)?;
                finish_row(&mut rows, &mut row)?;
                started = false;
                line += 1;
                if payload.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
            }
            _ => {
                field.push(byte);
                started = true;
            }
        }
        index += 1;
    }
    if quoted {
        return Err(CoreError::InvalidInput(format!("CSV line {line}: quoted cell is not closed")).into());
    }
    if after_quote || started || !field.is_empty() || !row.is_empty() {
        finish_field(&mut row, &mut field, line)?;
        finish_row(&mut rows, &mut row)?;
    }
    if rows.is_empty() {
        return Err(CoreError::InvalidInput("CSV file is empty".into()).into());
    }
    let width = rows[0].len();
    if let Some(index) = rows[0].iter().position(|header| header.trim().is_empty()) {
        return Err(CoreError::InvalidInput(format!("CSV header column {} is empty", index + 1)).into());
    }
    let mut unique = HashSet::new();
    if let Some(header) = rows[0]
        .iter()
        .find(|header| !unique.insert(header.trim().to_ascii_lowercase()))
    {
        return Err(CoreError::InvalidInput(format!(
            "CSV header '{}' is duplicated; every header must be unique",
            header.trim()
        ))
        .into());
    }
    if let Some((index, row)) = rows.iter().enumerate().find(|(_, row)| row.len() != width) {
        return Err(CoreError::InvalidInput(format!(
            "CSV row {} has {} columns; the header has {width}",
            index + 1,
            row.len()
        ))
        .into());
    }
    if rows.len() == 1 {
        return Err(CoreError::InvalidInput("CSV contains headers but no data rows".into()).into());
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_parser_handles_quotes_commas_and_multiline_cells() {
        let rows = parse_csv(b"title,tags,notes\r\nExample,\"one,two\",\"line 1\nline \"\"2\"\"\"\r\n")
            .expect("parse CSV");
        assert_eq!(rows[1], ["Example", "one,two", "line 1\nline \"2\""]);
    }

    #[test]
    fn canonical_round_trip_restores_formula_like_values() {
        let item = VaultItem {
            id: uuid::Uuid::new_v4(),
            item_type: VaultItemType::Login,
            title: "=SUM(A1:A2)".into(),
            username: Some("user@example.com".into()),
            password: Some("+secret".into()),
            url: None,
            notes: Some("line one\nline two".into()),
            tags: vec!["home,personal".into()],
            folder: Some("Email".into()),
            icon_data_url: None,
            security_questions: vec![SecurityQuestion {
                question: "First school?".into(),
                answer: "North".into(),
            }],
            custom_fields: vec![CustomField {
                label: "PIN".into(),
                value: "1234".into(),
                concealed: true,
            }],
            totp_secret: None,
            password_history: Vec::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let exported = export_csv_bytes(&[item]).expect("export");
        let preview = preview_csv_bytes(&exported).expect("preview");
        let (inputs, _) = import_inputs_from_csv(&exported, &preview.suggested_mapping).expect("import");
        assert_eq!(inputs[0].title, "=SUM(A1:A2)");
        assert_eq!(inputs[0].password.as_deref(), Some("+secret"));
        assert_eq!(inputs[0].tags, ["home,personal"]);
        assert_eq!(inputs[0].security_questions.len(), 1);
        assert_eq!(inputs[0].custom_fields.len(), 1);
    }

    #[test]
    fn mapped_import_can_preserve_unknown_columns_in_notes() {
        let payload = b"Name,Login,Favorite color\nExample,alice,green\n";
        let preview = preview_csv_bytes(payload).expect("preview");
        assert_eq!(preview.sample_rows, [["Example", "alice", "green"]]);
        let mut mapping = preview.suggested_mapping;
        mapping.include_unmapped_in_notes = true;
        let (inputs, _) = import_inputs_from_csv(payload, &mapping).expect("import");
        assert_eq!(
            inputs[0].notes.as_deref(),
            Some("Imported fields:\nFavorite color: green")
        );
    }

    #[test]
    fn mapped_import_limits_aggregate_notes_expansion() {
        let header = "H".repeat(95_000);
        let mut payload = format!("Name,{header}\n");
        for index in 0..354 {
            payload.push_str(&format!("item-{index},x\n"));
        }
        let preview = preview_csv_bytes(payload.as_bytes()).expect("preview");
        let mut mapping = preview.suggested_mapping;
        mapping.include_unmapped_in_notes = true;

        let error = import_inputs_from_csv(payload.as_bytes(), &mapping).expect_err("aggregate limit");
        assert!(error
            .to_string()
            .contains("import exceeds the 32 MiB aggregate Notes limit"));
    }

    #[test]
    fn comma_separated_tags_trim_surrounding_whitespace() {
        let payload = b"Name,Tags\nExample,\"food, drink,  rewards \"\n";
        let preview = preview_csv_bytes(payload).expect("preview");
        let (inputs, _) = import_inputs_from_csv(payload, &preview.suggested_mapping).expect("import");
        assert_eq!(inputs[0].tags, ["food", "drink", "rewards"]);
    }

    #[test]
    fn malformed_or_oversized_csv_is_rejected() {
        let quote = parse_csv(b"title,notes\nvalue,\"unterminated").expect_err("unterminated quote");
        assert!(quote
            .to_string()
            .contains("CSV line 2: quoted cell is not closed"));
        let duplicate = parse_csv(b"title,title\nleft,right\n").expect_err("duplicate header");
        assert!(duplicate.to_string().contains("header 'title' is duplicated"));
        let width = parse_csv(b"title,notes\nonly-one-cell\n").expect_err("row width");
        assert!(width
            .to_string()
            .contains("CSV row 2 has 1 columns; the header has 2"));
    }

    #[test]
    fn item_validation_errors_include_the_csv_row_and_cause() {
        let payload = format!("Name\n{}\n", "T".repeat(257));
        let preview = preview_csv_bytes(payload.as_bytes()).expect("preview");
        let error = import_inputs_from_csv(payload.as_bytes(), &preview.suggested_mapping)
            .expect_err("oversized title");
        let message = error.to_string();
        assert!(message.contains("CSV row 2"));
        assert!(message.contains("title/name exceeds 256 characters"));
    }
}
