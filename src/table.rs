use anyhow::{bail, Result};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Comma,
    Tab,
    Semicolon,
    Pipe,
    Colon,
    #[allow(dead_code)]
    Custom(u8),
}

impl Delimiter {
    pub fn as_byte(&self) -> u8 {
        match self {
            Delimiter::Comma => b',',
            Delimiter::Tab => b'\t',
            Delimiter::Semicolon => b';',
            Delimiter::Pipe => b'|',
            Delimiter::Colon => b':',
            Delimiter::Custom(b) => *b,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Delimiter::Comma => "CSV (,)",
            Delimiter::Tab => "TSV (\\t)",
            Delimiter::Semicolon => "Semicolon (;)",
            Delimiter::Pipe => "Pipe (|)",
            Delimiter::Colon => "Colon (:)",
            Delimiter::Custom(_) => "Custom",
        }
    }

    pub fn default_extension(&self, original_ext: Option<&str>) -> &'static str {
        if let Some(ext) = original_ext {
            match ext.to_lowercase().as_str() {
                "tsv" => return "tsv",
                "tab" => return "tab",
                "psv" => return "psv",
                "txt" => return "txt",
                "csv" => return "csv",
                _ => {}
            }
        }
        match self {
            Delimiter::Tab => "tsv",
            Delimiter::Pipe => "psv",
            Delimiter::Colon => "txt",
            Delimiter::Semicolon => "csv",
            _ => "csv",
        }
    }
}

/// Windows-1252 high-range bytes (0x80..=0x9F) mapped to their proper Unicode characters.
const CP1252_HIGH: [char; 32] = [
    '€', '\u{FFFD}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{FFFD}', 'Ž', '\u{FFFD}',
    '\u{FFFD}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{FFFD}', 'ž', 'Ÿ',
];

/// Converts a single Windows-1252 byte to a char.
pub fn cp1252_to_char(b: u8) -> char {
    match b {
        0x80..=0x9F => CP1252_HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// Decodes raw file bytes into a clean, normalized UTF-8 String, detecting UTF-8 (with/without BOM),
/// UTF-16 LE/BE, and Windows-1252/Latin-1. Also converts C1 control characters (U+0080..=U+009F)
/// resulting from legacy Windows-1252 -> Latin-1 transcoding back to their intended Unicode characters.
pub fn decode_file_bytes(bytes: &[u8]) -> (String, &'static str) {
    if bytes.is_empty() {
        return (String::new(), "Empty");
    }

    // 1. Check for UTF-8 BOM (0xEF, 0xBB, 0xBF)
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (s, _) = decode_utf8_with_mojibake_repair(&bytes[3..]);
        return (s, "UTF-8 (BOM stripped)");
    }

    // 2. Check for UTF-16 LE BOM (0xFF, 0xFE)
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let s = decode_utf16_le(&bytes[2..]);
        return (clean_control_characters(&s), "UTF-16 LE");
    }

    // 3. Check for UTF-16 BE BOM (0xFE, 0xFF)
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let s = decode_utf16_be(&bytes[2..]);
        return (clean_control_characters(&s), "UTF-16 BE");
    }

    // 4. Heuristic check for UTF-16 LE without BOM (alternating ASCII + 0x00)
    if bytes.len() >= 4 && bytes[1] == 0 && bytes[3] == 0 {
        let s = decode_utf16_le(bytes);
        return (clean_control_characters(&s), "UTF-16 LE (Heuristic)");
    }

    // 5. Try UTF-8 decoding
    if let Ok(valid_str) = std::str::from_utf8(bytes) {
        let (repaired, has_fixes) = repair_c1_and_mojibake(valid_str);
        let cleaned = clean_control_characters(&repaired);
        let label = if has_fixes {
            "UTF-8 (Windows-1252 C1 Repaired)"
        } else {
            "UTF-8"
        };
        return (cleaned, label);
    }

    // 6. Fallback: Mixed UTF-8 chunks with Windows-1252 byte-by-byte recovery
    let mut out = String::with_capacity(bytes.len() + 16);
    for chunk in bytes.utf8_chunks() {
        out.push_str(chunk.valid());
        for &b in chunk.invalid() {
            out.push(cp1252_to_char(b));
        }
    }
    let (repaired, _) = repair_c1_and_mojibake(&out);
    let cleaned = clean_control_characters(&repaired);
    (cleaned, "Windows-1252 / Mixed")
}

fn decode_utf16_le(bytes: &[u8]) -> String {
    let u16_iter = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]));
    char::decode_utf16(u16_iter)
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect()
}

fn decode_utf16_be(bytes: &[u8]) -> String {
    let u16_iter = bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]]));
    char::decode_utf16(u16_iter)
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect()
}

fn decode_utf8_with_mojibake_repair(bytes: &[u8]) -> (String, bool) {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return repair_c1_and_mojibake(s);
    }
    let mut out = String::with_capacity(bytes.len() + 8);
    for chunk in bytes.utf8_chunks() {
        out.push_str(chunk.valid());
        for &b in chunk.invalid() {
            out.push(cp1252_to_char(b));
        }
    }
    repair_c1_and_mojibake(&out)
}

/// Converts C1 control codes (U+0080..=U+009F) that occur when Windows-1252 files were
/// wrongly transcoded as ISO-8859-1 into UTF-8 back into the intended characters.
/// Also repairs double-encoded UTF-8 mojibake commonly found in CSV exports.
pub fn repair_c1_and_mojibake(input: &str) -> (String, bool) {
    let mut changed = false;
    let mut out = String::with_capacity(input.len());

    for ch in input.chars() {
        let code = ch as u32;
        if (0x80..=0x9F).contains(&code) {
            changed = true;
            out.push(CP1252_HIGH[(code - 0x80) as usize]);
        } else {
            out.push(ch);
        }
    }

    // Common UTF-8 mojibake fixes (e.g. "â€™" -> "’", "â€œ" -> "“", "â€\u{9d}" -> "”")
    if out.contains("â€") || out.contains("Â®") || out.contains("Â©") || out.contains("â„¢") {
        let replaced = out
            .replace("â€™", "’")
            .replace("â€˜", "‘")
            .replace("â€œ", "“")
            .replace("â€\u{9d}", "”")
            .replace("â€”", "—")
            .replace("â€“", "–")
            .replace("â„¢", "™")
            .replace("Â®", "®")
            .replace("Â©", "©");
        return (replaced, true);
    }

    (out, changed)
}

/// Strips terminal-breaking control characters (NUL, Bell, raw ESC) while preserving standard text whitespace.
pub fn clean_control_characters(input: &str) -> String {
    input
        .chars()
        .filter(|&c| c == '\t' || c == '\n' || c == '\r' || (c >= ' ' && c != '\x7F' && c != '\u{001B}'))
        .collect()
}

/// Sniffs the delimiter of a tabular file by analyzing column counts across lines outside quotes.
pub fn detect_delimiter(sample_text: &str, path_hint: Option<&Path>) -> Delimiter {
    let candidates = [
        (Delimiter::Tab, b'\t'),
        (Delimiter::Comma, b','),
        (Delimiter::Semicolon, b';'),
        (Delimiter::Pipe, b'|'),
        (Delimiter::Colon, b':'),
    ];

    // Priority hint based on file extension
    let ext_hint = path_hint
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase());

    let mut best_delimiter = Delimiter::Comma;
    let mut highest_score = -1.0;

    let lines: Vec<&str> = sample_text
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .take(50)
        .collect();

    if lines.is_empty() {
        if let Some(ref ext) = ext_hint {
            if ext == "tsv" || ext == "tab" {
                return Delimiter::Tab;
            } else if ext == "psv" {
                return Delimiter::Pipe;
            }
        }
        return Delimiter::Comma;
    }

    for (delim_enum, delim_byte) in candidates {
        let counts: Vec<usize> = lines
            .iter()
            .map(|line| count_delim_outside_quotes(line, delim_byte) + 1)
            .collect();

        let first_cols = counts[0];
        if first_cols < 2 {
            continue;
        }

        let total_lines = counts.len() as f64;
        let consistent_lines = counts.iter().filter(|&&c| c == first_cols).count() as f64;
        let consistency = consistent_lines / total_lines;

        let mut score = consistency * 100.0 + (first_cols as f64).min(20.0);

        // Boost score if file extension explicitly matches
        if let Some(ref ext) = ext_hint {
            match (delim_byte, ext.as_str()) {
                (b'\t', "tsv" | "tab") => score += 50.0,
                (b',', "csv") => score += 20.0,
                (b';', "csv") => score += 10.0,
                (b'|', "psv") => score += 50.0,
                _ => {}
            }
        }

        if score > highest_score {
            highest_score = score;
            best_delimiter = delim_enum;
        }
    }

    if highest_score < 0.0 {
        if let Some(ref ext) = ext_hint {
            match ext.as_str() {
                "tsv" | "tab" => return Delimiter::Tab,
                "psv" => return Delimiter::Pipe,
                _ => return Delimiter::Comma,
            }
        }
        return Delimiter::Comma;
    }

    best_delimiter
}

fn count_delim_outside_quotes(line: &str, delim: u8) -> usize {
    let delim_char = delim as char;
    let mut count = 0;
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                chars.next(); // Escaped quote
            } else {
                in_quotes = !in_quotes;
            }
        } else if c == delim_char && !in_quotes {
            count += 1;
        }
    }

    count
}

/// Parses tabular text using RFC 4180 parsing first, falling back to a resilient parser
/// if quote or syntax errors occur.
pub fn parse_tabular(text: &str, delimiter: Delimiter) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    if text.trim().is_empty() {
        bail!("File is empty");
    }

    let delim_byte = delimiter.as_byte();

    // Fast path: Try standard csv reader
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim_byte)
        .flexible(true)
        .has_headers(false)
        .double_quote(true)
        .from_reader(text.as_bytes());

    let mut records = Vec::new();
    let mut syntax_error = false;

    for result in rdr.records() {
        match result {
            Ok(rec) => {
                records.push(rec.iter().map(|s| s.to_string()).collect::<Vec<String>>());
            }
            Err(_) => {
                syntax_error = true;
                break;
            }
        }
    }

    if !syntax_error && !records.is_empty() {
        let headers = records.remove(0);
        let num_cols = headers.len();
        if num_cols > 0 {
            for row in &mut records {
                if row.len() < num_cols {
                    row.resize(num_cols, String::new());
                }
            }
            return Ok((headers, records));
        }
    }

    // Fallback path: Resilient character-by-character parser that tolerates unescaped quotes & irregular rows
    let (headers, rows) = parse_resilient(text, delim_byte);
    if headers.is_empty() {
        bail!("No header row found in file");
    }

    Ok((headers, rows))
}

/// A tolerant parser that handles unescaped quotes (e.g. 10" Banjo Rim), unclosed quotes,
/// and mixed newlines without crashing.
pub fn parse_resilient(text: &str, delim: u8) -> (Vec<String>, Vec<Vec<String>>) {
    let delim_char = delim as char;
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    // Escaped quote "" -> "
                    chars.next();
                    current_field.push('"');
                } else {
                    let next_is_boundary = match chars.peek() {
                        Some(&ch) => ch == delim_char || ch == '\n' || ch == '\r',
                        None => true,
                    };
                    if next_is_boundary {
                        in_quotes = false;
                    } else {
                        // Mid-field quote in resilient mode: keep literal quote
                        current_field.push('"');
                    }
                }
            } else {
                current_field.push(c);
            }
        } else if c == '"' {
            if current_field.trim().is_empty() {
                in_quotes = true;
                current_field.clear();
            } else {
                // Quote in unquoted field (like 8" Rim)
                current_field.push('"');
            }
        } else if c == delim_char {
            current_row.push(std::mem::take(&mut current_field));
        } else if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            current_row.push(std::mem::take(&mut current_field));
            if !current_row.iter().all(|f| f.is_empty()) || !rows.is_empty() {
                rows.push(std::mem::take(&mut current_row));
            } else {
                current_row.clear();
            }
        } else if c == '\n' {
            current_row.push(std::mem::take(&mut current_field));
            if !current_row.iter().all(|f| f.is_empty()) || !rows.is_empty() {
                rows.push(std::mem::take(&mut current_row));
            } else {
                current_row.clear();
            }
        } else {
            current_field.push(c);
        }
    }

    if !current_field.is_empty() || !current_row.is_empty() {
        current_row.push(current_field);
        if !current_row.iter().all(|f| f.is_empty()) {
            rows.push(current_row);
        }
    }

    if rows.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let headers = rows.remove(0);
    let num_cols = headers.len();

    for row in &mut rows {
        if row.len() < num_cols {
            row.resize(num_cols, String::new());
        }
    }

    (headers, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c1_control_repair() {
        // U+0096 is C1 control for CP1252 0x96 (en dash '–')
        // U+0093 is C1 control for CP1252 0x93 (left double quote '“')
        let bad_utf8 = "Model\u{0096}Special: \u{0093}Vintage\u{0094} banjo";
        let (repaired, changed) = repair_c1_and_mojibake(bad_utf8);
        assert!(changed);
        assert_eq!(repaired, "Model–Special: “Vintage” banjo");
    }

    #[test]
    fn test_utf16_le_decode() {
        let text = "id\tname\n1\tBanjo\n";
        let mut bytes = vec![0xFF, 0xFE]; // LE BOM
        for c in text.encode_utf16() {
            bytes.extend_from_slice(&c.to_le_bytes());
        }

        let (decoded, enc) = decode_file_bytes(&bytes);
        assert_eq!(enc, "UTF-16 LE");
        assert_eq!(decoded, text);
    }

    #[test]
    fn test_utf8_bom_strip() {
        let text = "id,name\n1,Banjo\n";
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(text.as_bytes());

        let (decoded, enc) = decode_file_bytes(&bytes);
        assert_eq!(enc, "UTF-8 (BOM stripped)");
        assert_eq!(decoded, text);
    }

    #[test]
    fn test_detect_delimiters() {
        let csv_data = "id,title,price\n1,Banjo,599\n2,Uke,299\n";
        assert_eq!(detect_delimiter(csv_data, None), Delimiter::Comma);

        let tsv_data = "id\ttitle\tprice\n1\tBanjo\t599\n2\tUke\t299\n";
        assert_eq!(detect_delimiter(tsv_data, None), Delimiter::Tab);

        let semi_data = "id;title;price\n1;Banjo;599\n2;Uke;299\n";
        assert_eq!(detect_delimiter(semi_data, None), Delimiter::Semicolon);

        let pipe_data = "id|title|price\n1|Banjo|599\n2|Uke|299\n";
        assert_eq!(detect_delimiter(pipe_data, None), Delimiter::Pipe);
    }

    #[test]
    fn test_resilient_unescaped_quotes() {
        // Field 2 has unescaped quotes: 10" Banjo Rim
        let data = "id,desc,price\n1,10\" Banjo Rim,599\n2,\"11\"\" Maple\",699\n";
        let (headers, rows) = parse_tabular(data, Delimiter::Comma).unwrap();
        assert_eq!(headers, vec!["id", "desc", "price"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][1], "10\" Banjo Rim");
        assert_eq!(rows[1][1], "11\" Maple");
    }

    #[test]
    fn test_ragged_rows_padding() {
        let data = "col1,col2,col3\nval1\nval1,val2,val3,val4\n";
        let (headers, rows) = parse_tabular(data, Delimiter::Comma).unwrap();
        assert_eq!(headers.len(), 3);
        assert_eq!(rows[0].len(), 3);
        assert_eq!(rows[0][0], "val1");
        assert_eq!(rows[0][1], "");
        assert_eq!(rows[0][2], "");
    }
}
