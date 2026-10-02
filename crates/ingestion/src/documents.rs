//! Text extraction and previews for business documents: Word, Excel,
//! PowerPoint, PDF, OneNote and Outlook messages.
//!
//! Everything here only reads bytes. Nothing is executed, no macros run and no
//! external references are followed. Archive members are read through the
//! bounded ZIP reader in the parent module, so a hostile file cannot expand
//! without limit.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{bail, Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use repomemo_domain::{
    DocumentPreview, EmailAttachmentInfo, SheetPreview, SlidePreview,
};

use super::{extension, extract_word_text, read_zip_member, zip_entry_names};

const MAX_SHEET_ROWS: usize = 50_000;
const MAX_SHEET_COLUMNS: usize = 256;
const MAX_CELL_CHARS: usize = 2_000;
const MAX_SHEETS: usize = 50;
const MAX_SLIDES: usize = 500;
const MAX_INDEX_CHARS: usize = 3 * 1024 * 1024;
const MAX_STREAM_BYTES: u64 = 8 * 1024 * 1024;

const PREVIEW_ROWS: usize = 100;
const PREVIEW_COLUMNS: usize = 30;
const PREVIEW_CELL_CHARS: usize = 200;
const PREVIEW_SHEETS: usize = 10;
const PREVIEW_SLIDES: usize = 100;
const PREVIEW_TEXT_CHARS: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    Word,
    Excel,
    PowerPoint,
    Pdf,
    OneNote,
    Email,
}

impl DocumentKind {
    /// The label stored as the artifact language and shown in the interface.
    pub fn label(self) -> &'static str {
        match self {
            Self::Word => "Word",
            Self::Excel => "Excel",
            Self::PowerPoint => "PowerPoint",
            Self::Pdf => "PDF",
            Self::OneNote => "OneNote",
            Self::Email => "Email",
        }
    }
}

pub fn document_kind(path: &Path) -> Option<DocumentKind> {
    match extension(path)?.as_str() {
        "doc" | "docx" => Some(DocumentKind::Word),
        "xlsx" | "xlsm" | "xls" => Some(DocumentKind::Excel),
        "pptx" | "ppt" => Some(DocumentKind::PowerPoint),
        "pdf" => Some(DocumentKind::Pdf),
        "one" => Some(DocumentKind::OneNote),
        "eml" | "msg" => Some(DocumentKind::Email),
        _ => None,
    }
}

pub fn is_document(path: &Path) -> bool {
    document_kind(path).is_some()
}

pub fn document_extensions() -> &'static [&'static str] {
    &[
        "doc", "docx", "xlsx", "xlsm", "xls", "pptx", "ppt", "pdf", "one", "eml", "msg",
    ]
}

/// Searchable text for any supported business document, or `None` for other
/// file types. A document with no readable text (for example a scanned PDF)
/// yields an empty string rather than an error.
pub fn extract_document_text(path: &Path, bytes: &[u8]) -> Result<Option<String>> {
    let Some(kind) = document_kind(path) else {
        return Ok(None);
    };
    let text = match kind {
        DocumentKind::Word => return extract_word_text(path, bytes),
        DocumentKind::Excel => sheets_to_text(&read_sheets(path, bytes)?),
        DocumentKind::PowerPoint => match extension(path).as_deref() {
            Some("ppt") => legacy_ppt_text(bytes)?,
            _ => slides_to_text(&read_slides(bytes)?),
        },
        DocumentKind::Pdf => pdf_text(bytes)?,
        DocumentKind::OneNote => onenote_text(bytes),
        DocumentKind::Email => email_to_text(&read_email(path, bytes)?),
    };
    Ok(Some(cap_chars(text, MAX_INDEX_CHARS)))
}

/// A preview for the reader. Never fails: a file that cannot be read yields
/// `Unavailable` with the reason, so the client can still offer to open it.
pub fn document_preview(path: &Path, bytes: &[u8]) -> DocumentPreview {
    let Some(kind) = document_kind(path) else {
        return unavailable("This file type has no preview.");
    };
    let result: Result<DocumentPreview> = (|| match kind {
        DocumentKind::Word => {
            let text = extract_word_text(path, bytes)?.unwrap_or_default();
            Ok(text_preview(text, false))
        }
        DocumentKind::Excel => {
            let sheets = read_sheets(path, bytes)?;
            let total_sheets = sheets.len();
            Ok(DocumentPreview::Sheets {
                sheets: sheets
                    .into_iter()
                    .take(PREVIEW_SHEETS)
                    .map(sheet_preview)
                    .collect(),
                total_sheets,
            })
        }
        DocumentKind::PowerPoint => {
            if extension(path).as_deref() == Some("ppt") {
                return Ok(text_preview(legacy_ppt_text(bytes)?, true));
            }
            let slides = read_slides(bytes)?;
            let total_slides = slides.len();
            Ok(DocumentPreview::Slides {
                slides: slides.into_iter().take(PREVIEW_SLIDES).collect(),
                total_slides,
            })
        }
        DocumentKind::Pdf => Ok(DocumentPreview::Pdf {
            page_count: pdf_page_count(bytes),
        }),
        DocumentKind::OneNote => Ok(text_preview(onenote_text(bytes), true)),
        DocumentKind::Email => {
            let email = read_email(path, bytes)?;
            let (body, truncated) = truncate_chars(&email.body, PREVIEW_TEXT_CHARS);
            Ok(DocumentPreview::Email {
                subject: email.subject,
                from: email.from,
                to: email.to,
                cc: email.cc,
                date: email.date,
                body,
                truncated,
                attachments: email.attachments,
            })
        }
    })();
    result.unwrap_or_else(|error| unavailable(&format!("{error:#}")))
}

fn unavailable(reason: &str) -> DocumentPreview {
    DocumentPreview::Unavailable {
        reason: reason.to_owned(),
    }
}

fn text_preview(text: String, approximate: bool) -> DocumentPreview {
    let (text, truncated) = truncate_chars(&text, PREVIEW_TEXT_CHARS);
    DocumentPreview::Text {
        text,
        truncated,
        approximate,
    }
}

fn truncate_chars(value: &str, limit: usize) -> (String, bool) {
    if value.chars().count() <= limit {
        return (value.to_owned(), false);
    }
    (value.chars().take(limit).collect(), true)
}

fn cap_chars(value: String, limit: usize) -> String {
    truncate_chars(&value, limit).0
}

// ---------------------------------------------------------------------------
// XML helpers
// ---------------------------------------------------------------------------

fn local(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn attribute(event: &BytesStart, key: &[u8]) -> Option<String> {
    event.attributes().flatten().find_map(|attribute| {
        (local(attribute.key.as_ref()) == key)
            .then(|| attribute.unescape_value().ok().map(|value| value.into_owned()))
            .flatten()
    })
}

fn push_reference(output: &mut String, reference: &quick_xml::events::BytesRef) {
    if let Ok(Some(character)) = reference.resolve_char_ref() {
        output.push(character);
        return;
    }
    if let Ok(name) = reference.decode() {
        if let Some(resolved) = quick_xml::escape::resolve_predefined_entity(&name) {
            output.push_str(resolved);
        }
    }
}

fn xml_reader(bytes: &[u8]) -> Reader<&[u8]> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    reader
}

// ---------------------------------------------------------------------------
// Excel
// ---------------------------------------------------------------------------

struct ParsedSheet {
    name: String,
    rows: Vec<Vec<String>>,
}

fn read_sheets(path: &Path, bytes: &[u8]) -> Result<Vec<ParsedSheet>> {
    match extension(path).as_deref() {
        Some("xls") => read_legacy_xls(bytes),
        _ => read_xlsx(bytes),
    }
}

fn read_legacy_xls(bytes: &[u8]) -> Result<Vec<ParsedSheet>> {
    use calamine::{Data, Reader as _, Xls};
    let mut workbook: Xls<_> = calamine::open_workbook_from_rs(Cursor::new(bytes))
        .context("could not read the legacy Excel workbook")?;
    let mut sheets = Vec::new();
    for (name, range) in workbook.worksheets().into_iter().take(MAX_SHEETS) {
        let mut rows = Vec::new();
        for row in range.rows().take(MAX_SHEET_ROWS) {
            rows.push(
                row.iter()
                    .take(MAX_SHEET_COLUMNS)
                    .map(|cell| match cell {
                        Data::Empty => String::new(),
                        Data::DateTime(value) => excel_serial_text(value.as_f64()),
                        other => clip(other.to_string(), MAX_CELL_CHARS),
                    })
                    .collect::<Vec<_>>(),
            );
        }
        trim_trailing_empty_rows(&mut rows);
        sheets.push(ParsedSheet { name, rows });
    }
    Ok(sheets)
}

fn read_xlsx(bytes: &[u8]) -> Result<Vec<ParsedSheet>> {
    let workbook = read_zip_member(bytes, "xl/workbook.xml")
        .context("this does not look like an Excel workbook")?;
    let relationships = read_zip_member(bytes, "xl/_rels/workbook.xml.rels")?;
    let declared = workbook_sheets(&workbook)?;
    let targets = relationship_targets(&relationships)?;
    let shared = match read_zip_member(bytes, "xl/sharedStrings.xml") {
        Ok(xml) => shared_strings(&xml)?,
        Err(_) => Vec::new(),
    };
    let date_styles = read_zip_member(bytes, "xl/styles.xml")
        .map(|xml| date_styles(&xml))
        .unwrap_or_default();

    let mut sheets = Vec::new();
    for (name, relationship) in declared.into_iter().take(MAX_SHEETS) {
        let Some(target) = targets.get(&relationship) else {
            continue;
        };
        let member = match target.strip_prefix('/') {
            Some(absolute) => absolute.to_owned(),
            None => format!("xl/{target}"),
        };
        let Ok(xml) = read_zip_member(bytes, &member) else {
            continue;
        };
        let rows = parse_sheet_rows(&xml, &shared, &date_styles)?;
        sheets.push(ParsedSheet { name, rows });
    }
    if sheets.is_empty() {
        bail!("The workbook has no readable sheets.");
    }
    Ok(sheets)
}

fn workbook_sheets(xml: &[u8]) -> Result<Vec<(String, String)>> {
    let mut reader = xml_reader(xml);
    let mut buffer = Vec::new();
    let mut sheets = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if local(event.name().as_ref()) == b"sheet" =>
            {
                if let (Some(name), Some(id)) =
                    (attribute(&event, b"name"), attribute(&event, b"id"))
                {
                    sheets.push((name, id));
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => bail!("invalid workbook.xml: {error}"),
            _ => {}
        }
        buffer.clear();
    }
    Ok(sheets)
}

fn relationship_targets(xml: &[u8]) -> Result<HashMap<String, String>> {
    let mut reader = xml_reader(xml);
    let mut buffer = Vec::new();
    let mut targets = HashMap::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if local(event.name().as_ref()) == b"Relationship" =>
            {
                if let (Some(id), Some(target)) =
                    (attribute(&event, b"Id"), attribute(&event, b"Target"))
                {
                    targets.insert(id, target);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => bail!("invalid relationships: {error}"),
            _ => {}
        }
        buffer.clear();
    }
    Ok(targets)
}

fn shared_strings(xml: &[u8]) -> Result<Vec<String>> {
    let mut reader = xml_reader(xml);
    let mut buffer = Vec::new();
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut in_item = false;
    let mut in_text = false;
    let mut in_phonetic = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => match local(event.name().as_ref()) {
                b"si" => {
                    in_item = true;
                    current.clear();
                }
                b"rPh" => in_phonetic = true,
                b"t" if in_item && !in_phonetic => in_text = true,
                _ => {}
            },
            Ok(Event::End(event)) => match local(event.name().as_ref()) {
                b"si" => {
                    in_item = false;
                    strings.push(clip(std::mem::take(&mut current), MAX_CELL_CHARS));
                }
                b"rPh" => in_phonetic = false,
                b"t" => in_text = false,
                _ => {}
            },
            Ok(Event::Text(text)) if in_text => {
                current.push_str(&text.decode().context("invalid shared string")?)
            }
            Ok(Event::GeneralRef(reference)) if in_text => {
                push_reference(&mut current, &reference)
            }
            Ok(Event::Eof) => break,
            Err(error) => bail!("invalid sharedStrings.xml: {error}"),
            _ => {}
        }
        buffer.clear();
    }
    Ok(strings)
}

/// For each cell style index, whether it displays a date or time.
fn date_styles(xml: &[u8]) -> Vec<bool> {
    let mut reader = xml_reader(xml);
    let mut buffer = Vec::new();
    let mut custom: HashMap<u32, String> = HashMap::new();
    let mut styles = Vec::new();
    let mut in_cell_xfs = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                match local(event.name().as_ref()) {
                    b"numFmt" => {
                        if let (Some(id), Some(code)) =
                            (attribute(&event, b"numFmtId"), attribute(&event, b"formatCode"))
                        {
                            if let Ok(id) = id.parse() {
                                custom.insert(id, code);
                            }
                        }
                    }
                    b"cellXfs" => in_cell_xfs = true,
                    b"xf" if in_cell_xfs => {
                        let id = attribute(&event, b"numFmtId")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(0);
                        styles.push(is_date_format(id, custom.get(&id).map(String::as_str)));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(event)) if local(event.name().as_ref()) == b"cellXfs" => {
                in_cell_xfs = false
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }
    styles
}

fn is_date_format(id: u32, code: Option<&str>) -> bool {
    if matches!(id, 14..=22 | 27..=36 | 45..=47 | 50..=58) {
        return true;
    }
    let Some(code) = code else {
        return false;
    };
    let mut stripped = String::new();
    let mut quoted = false;
    let mut bracketed = false;
    for character in code.chars() {
        match character {
            '"' => quoted = !quoted,
            '[' if !quoted => bracketed = true,
            ']' if !quoted => bracketed = false,
            _ if quoted || bracketed => {}
            _ => stripped.push(character.to_ascii_lowercase()),
        }
    }
    stripped.contains(['y', 'd', 'h', 's'])
        || (stripped.contains('m') && !stripped.contains(['0', '#']))
}

fn column_index(reference: &str) -> Option<usize> {
    let letters = reference
        .chars()
        .take_while(|character| character.is_ascii_alphabetic())
        .collect::<String>();
    if letters.is_empty() {
        return None;
    }
    let mut index = 0_usize;
    for character in letters.chars() {
        index = index * 26 + (character.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    Some(index - 1)
}

fn parse_sheet_rows(
    xml: &[u8],
    shared: &[String],
    date_styles: &[bool],
) -> Result<Vec<Vec<String>>> {
    let mut reader = xml_reader(xml);
    let mut buffer = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row_index = 0_usize;
    let mut next_column = 0_usize;

    let mut cell_type = String::new();
    let mut cell_style = 0_usize;
    let mut cell_column = 0_usize;
    let mut value = String::new();
    let mut in_cell = false;
    let mut in_value = false;
    let mut in_inline_text = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => match local(event.name().as_ref()) {
                b"row" => {
                    row_index = attribute(&event, b"r")
                        .and_then(|value| value.parse::<usize>().ok())
                        .map(|number| number.saturating_sub(1))
                        .unwrap_or(rows.len());
                    next_column = 0;
                }
                b"c" => {
                    in_cell = true;
                    value.clear();
                    cell_type = attribute(&event, b"t").unwrap_or_default();
                    cell_style = attribute(&event, b"s")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0);
                    cell_column = attribute(&event, b"r")
                        .and_then(|reference| column_index(&reference))
                        .unwrap_or(next_column);
                }
                b"v" if in_cell => in_value = true,
                b"t" if in_cell => in_inline_text = true,
                _ => {}
            },
            Ok(Event::Text(text)) if in_value || in_inline_text => {
                value.push_str(&text.decode().context("invalid cell text")?)
            }
            Ok(Event::GeneralRef(reference)) if in_value || in_inline_text => {
                push_reference(&mut value, &reference)
            }
            Ok(Event::End(event)) => match local(event.name().as_ref()) {
                b"v" => in_value = false,
                b"t" => in_inline_text = false,
                b"c" => {
                    in_cell = false;
                    next_column = cell_column + 1;
                    if row_index >= MAX_SHEET_ROWS || cell_column >= MAX_SHEET_COLUMNS {
                        buffer.clear();
                        continue;
                    }
                    let text = render_cell(
                        &cell_type,
                        &value,
                        shared,
                        date_styles.get(cell_style).copied().unwrap_or(false),
                    );
                    if text.is_empty() {
                        buffer.clear();
                        continue;
                    }
                    if rows.len() <= row_index {
                        rows.resize_with(row_index + 1, Vec::new);
                    }
                    let row = &mut rows[row_index];
                    if row.len() <= cell_column {
                        row.resize(cell_column + 1, String::new());
                    }
                    row[cell_column] = clip(text, MAX_CELL_CHARS);
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => bail!("invalid worksheet XML: {error}"),
            _ => {}
        }
        buffer.clear();
    }
    trim_trailing_empty_rows(&mut rows);
    Ok(rows)
}

fn render_cell(cell_type: &str, value: &str, shared: &[String], is_date: bool) -> String {
    match cell_type {
        "s" => value
            .trim()
            .parse::<usize>()
            .ok()
            .and_then(|index| shared.get(index))
            .cloned()
            .unwrap_or_default(),
        "str" | "inlineStr" | "e" | "d" => value.to_owned(),
        "b" => if value.trim() == "1" { "TRUE" } else { "FALSE" }.to_owned(),
        _ => {
            let trimmed = value.trim();
            let Ok(number) = trimmed.parse::<f64>() else {
                return trimmed.to_owned();
            };
            if is_date {
                excel_serial_text(number)
            } else if number.is_finite() && number.fract() == 0.0 && number.abs() < 1e15 {
                format!("{}", number as i64)
            } else {
                trimmed.to_owned()
            }
        }
    }
}

/// Converts an Excel date serial (1900 system) into readable text.
fn excel_serial_text(serial: f64) -> String {
    if !serial.is_finite() || serial < 0.0 {
        return serial.to_string();
    }
    let day_number = serial.floor() as i64;
    let seconds = ((serial - serial.floor()) * 86_400.0).round() as i64;
    let time = format!("{:02}:{:02}", seconds / 3600, (seconds % 3600) / 60);
    if day_number == 0 {
        return time;
    }
    // Days since 1899-12-30 (Excel's epoch, which absorbs its 1900 leap bug).
    let (year, month, day) = civil_from_days(day_number - 25_569);
    let date = format!("{year:04}-{month:02}-{day:02}");
    if seconds == 0 {
        date
    } else {
        format!("{date} {time}")
    }
}

/// Days since 1970-01-01 to a proleptic Gregorian date.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_part + 2) / 5 + 1) as u32;
    let month = if month_part < 10 { month_part + 3 } else { month_part - 9 } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn trim_trailing_empty_rows(rows: &mut Vec<Vec<String>>) {
    while rows
        .last()
        .is_some_and(|row| row.iter().all(String::is_empty))
    {
        rows.pop();
    }
}

fn clip(value: String, limit: usize) -> String {
    if value.chars().count() <= limit {
        value
    } else {
        value.chars().take(limit).collect()
    }
}

fn sheet_preview(sheet: ParsedSheet) -> SheetPreview {
    let total_rows = sheet.rows.len();
    let total_columns = sheet.rows.iter().map(Vec::len).max().unwrap_or(0);
    let rows = sheet
        .rows
        .into_iter()
        .take(PREVIEW_ROWS)
        .map(|row| {
            let mut cells = row
                .into_iter()
                .take(PREVIEW_COLUMNS)
                .map(|cell| clip(cell, PREVIEW_CELL_CHARS))
                .collect::<Vec<_>>();
            cells.resize(total_columns.min(PREVIEW_COLUMNS), String::new());
            cells
        })
        .collect();
    SheetPreview {
        name: sheet.name,
        rows,
        total_rows,
        total_columns,
    }
}

fn sheets_to_text(sheets: &[ParsedSheet]) -> String {
    let mut text = String::new();
    for sheet in sheets {
        text.push_str(&format!("Sheet: {}\n", sheet.name));
        for row in &sheet.rows {
            let cells = row
                .iter()
                .filter(|cell| !cell.is_empty())
                .map(String::as_str)
                .collect::<Vec<_>>();
            if !cells.is_empty() {
                text.push_str(&cells.join(" | "));
                text.push('\n');
            }
        }
        text.push('\n');
        if text.len() > MAX_INDEX_CHARS {
            break;
        }
    }
    text.trim().to_owned()
}

// ---------------------------------------------------------------------------
// PowerPoint
// ---------------------------------------------------------------------------

fn read_slides(bytes: &[u8]) -> Result<Vec<SlidePreview>> {
    let mut numbered = zip_entry_names(bytes)?
        .into_iter()
        .filter_map(|name| {
            let number = name
                .strip_prefix("ppt/slides/slide")?
                .strip_suffix(".xml")?
                .parse::<usize>()
                .ok()?;
            Some((number, name))
        })
        .collect::<Vec<_>>();
    if numbered.is_empty() {
        bail!("This does not look like a PowerPoint presentation.");
    }
    numbered.sort();
    numbered.truncate(MAX_SLIDES);

    let mut slides = Vec::new();
    for (number, member) in numbered {
        let xml = read_zip_member(bytes, &member)?;
        let (title, text) = slide_paragraphs(&xml, false)?;
        let notes = read_zip_member(bytes, &format!("ppt/notesSlides/notesSlide{number}.xml"))
            .ok()
            .and_then(|xml| slide_paragraphs(&xml, true).ok())
            .map(|(_, paragraphs)| paragraphs.join("\n"))
            .filter(|notes| !notes.trim().is_empty());
        slides.push(SlidePreview {
            number,
            title,
            text,
            notes,
        });
    }
    Ok(slides)
}

/// Paragraph text of a slide, with its title separated. For a notes page only
/// the body placeholder is kept (not the slide image or page number).
fn slide_paragraphs(xml: &[u8], notes_page: bool) -> Result<(Option<String>, Vec<String>)> {
    let mut reader = xml_reader(xml);
    let mut buffer = Vec::new();
    let mut title = None;
    let mut paragraphs = Vec::new();

    let mut placeholder: Option<String> = None;
    let mut paragraph = String::new();
    let mut in_paragraph = false;
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if local(event.name().as_ref()) == b"ph" =>
            {
                placeholder = Some(attribute(&event, b"type").unwrap_or_else(|| "body".into()));
            }
            Ok(Event::Start(event)) => match local(event.name().as_ref()) {
                b"sp" | b"graphicFrame" => placeholder = None,
                b"p" => {
                    in_paragraph = true;
                    paragraph.clear();
                }
                b"t" if in_paragraph => in_text = true,
                b"br" if in_paragraph => paragraph.push('\n'),
                _ => {}
            },
            Ok(Event::Text(text)) if in_text => {
                paragraph.push_str(&text.decode().context("invalid slide text")?)
            }
            Ok(Event::GeneralRef(reference)) if in_text => push_reference(&mut paragraph, &reference),
            Ok(Event::End(event)) => match local(event.name().as_ref()) {
                b"t" => in_text = false,
                b"p" => {
                    in_paragraph = false;
                    let value = paragraph.trim().to_owned();
                    if value.is_empty() {
                        buffer.clear();
                        continue;
                    }
                    let kind = placeholder.as_deref().unwrap_or("");
                    let skipped = if notes_page {
                        kind != "body"
                    } else {
                        matches!(kind, "sldNum" | "dt" | "ftr" | "hdr")
                    };
                    if skipped {
                        // not slide content
                    } else if !notes_page
                        && matches!(kind, "title" | "ctrTitle")
                        && title.is_none()
                    {
                        title = Some(value);
                    } else {
                        paragraphs.push(value);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => bail!("invalid slide XML: {error}"),
            _ => {}
        }
        buffer.clear();
    }
    Ok((title, paragraphs))
}

fn slides_to_text(slides: &[SlidePreview]) -> String {
    let mut text = String::new();
    for slide in slides {
        text.push_str(&format!("Slide {}", slide.number));
        if let Some(title) = &slide.title {
            text.push_str(&format!(": {title}"));
        }
        text.push('\n');
        for line in &slide.text {
            text.push_str(line);
            text.push('\n');
        }
        if let Some(notes) = &slide.notes {
            text.push_str("Speaker notes: ");
            text.push_str(notes);
            text.push('\n');
        }
        text.push('\n');
    }
    text.trim().to_owned()
}

/// Legacy .ppt: text lives in TextCharsAtom (UTF-16) and TextBytesAtom
/// (Latin-1) records of the "PowerPoint Document" stream.
fn legacy_ppt_text(bytes: &[u8]) -> Result<String> {
    let mut compound = cfb::CompoundFile::open(Cursor::new(bytes))
        .context("legacy .ppt files must use the Microsoft Compound File format")?;
    let stream = compound
        .open_stream("/PowerPoint Document")
        .context("legacy .ppt is missing its PowerPoint Document stream")?;
    let mut data = Vec::new();
    stream.take(MAX_STREAM_BYTES).read_to_end(&mut data)?;

    let mut text = String::new();
    let mut offset = 0;
    while offset + 8 <= data.len() {
        let version = data[offset] & 0x0f;
        let record_type = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        let length = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;
        offset += 8;
        if version == 0x0f {
            continue; // a container: step inside it
        }
        let Some(body) = data.get(offset..offset.saturating_add(length)) else {
            break;
        };
        match record_type {
            0x0fa0 => {
                let units = body
                    .chunks_exact(2)
                    .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                    .collect::<Vec<_>>();
                text.push_str(&String::from_utf16_lossy(&units));
                text.push('\n');
            }
            0x0fa8 => {
                text.extend(body.iter().map(|byte| *byte as char));
                text.push('\n');
            }
            _ => {}
        }
        offset += length;
    }
    Ok(text
        .replace('\r', "\n")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}

// ---------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------

fn pdf_text(bytes: &[u8]) -> Result<String> {
    let outcome = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(bytes));
    match outcome {
        Ok(Ok(text)) => Ok(text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")),
        Ok(Err(error)) => {
            // Scanned or encrypted PDFs have no extractable text: keep them
            // stored and viewable instead of failing indexing forever.
            let _ = error;
            if bytes.starts_with(b"%PDF") {
                Ok(String::new())
            } else {
                bail!("This file is not a valid PDF.")
            }
        }
        Err(_) => Ok(String::new()),
    }
}

fn pdf_page_count(bytes: &[u8]) -> Option<usize> {
    std::panic::catch_unwind(|| pdf_extract::Document::load_mem(bytes).ok())
        .ok()
        .flatten()
        .map(|document| document.get_pages().len())
}

// ---------------------------------------------------------------------------
// OneNote
// ---------------------------------------------------------------------------

/// OneNote's .one format is a proprietary binary graph. Page text is stored as
/// UTF-16 strings, so this recovers readable runs. It is approximate: it can
/// miss text and may include a few labels that are not page content.
fn onenote_text(bytes: &[u8]) -> String {
    let mut runs: Vec<String> = Vec::new();
    let mut current: Vec<u16> = Vec::new();
    let mut index = 0;
    while index + 1 < bytes.len() {
        let unit = u16::from_le_bytes([bytes[index], bytes[index + 1]]);
        // Latin, Greek and Cyrillic text plus common punctuation. Wider ranges
        // would also match misaligned binary, so other scripts are not recovered.
        let printable = matches!(
            unit,
            0x09 | 0x0a | 0x0d | 0x20..=0x7e | 0xa0..=0x24f | 0x370..=0x52f | 0x2013..=0x2026 | 0x20ac
        );
        if printable {
            current.push(unit);
            index += 2;
        } else {
            flush_onenote_run(&mut current, &mut runs);
            // Strings are 2-byte aligned relative to each other, but record
            // boundaries are not, so advance one byte on a miss.
            index += 1;
        }
    }
    flush_onenote_run(&mut current, &mut runs);
    runs.dedup();
    runs.join("\n")
}

fn flush_onenote_run(current: &mut Vec<u16>, runs: &mut Vec<String>) {
    if current.len() >= 8 {
        let text = String::from_utf16_lossy(current)
            .replace('\r', "\n")
            .trim()
            .to_owned();
        let letters = text.chars().filter(|c| c.is_alphabetic()).count();
        let total = text.chars().count();
        let has_word = text
            .split_whitespace()
            .any(|word| word.chars().filter(|c| c.is_alphabetic()).count() >= 3);
        let looks_like_id = text.starts_with('{') || text.chars().filter(|c| *c == '-').count() > 3;
        if total > 0 && letters * 100 / total >= 55 && has_word && !looks_like_id {
            runs.push(text);
        }
    }
    current.clear();
}

// ---------------------------------------------------------------------------
// Outlook messages
// ---------------------------------------------------------------------------

struct EmailData {
    subject: Option<String>,
    from: Option<String>,
    to: Vec<String>,
    cc: Vec<String>,
    date: Option<String>,
    body: String,
    attachments: Vec<EmailAttachmentInfo>,
}

fn read_email(path: &Path, bytes: &[u8]) -> Result<EmailData> {
    match extension(path).as_deref() {
        Some("msg") => read_msg(bytes),
        _ => read_eml(bytes),
    }
}

fn read_eml(bytes: &[u8]) -> Result<EmailData> {
    use mail_parser::{Address, MessageParser, MimeHeaders};
    let message = MessageParser::default()
        .parse(bytes)
        .context("could not parse this e-mail message")?;

    fn people(address: Option<&Address>) -> Vec<String> {
        address
            .map(|address| {
                address
                    .iter()
                    .map(|addr| match (addr.name(), addr.address()) {
                        (Some(name), Some(email)) => format!("{name} <{email}>"),
                        (None, Some(email)) => email.to_owned(),
                        (Some(name), None) => name.to_owned(),
                        (None, None) => String::new(),
                    })
                    .filter(|entry| !entry.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    let body = message
        .body_text(0)
        .map(|text| text.into_owned())
        .unwrap_or_default();
    let attachments = message
        .attachments()
        .map(|part| EmailAttachmentInfo {
            name: part
                .attachment_name()
                .map(str::to_owned)
                .unwrap_or_else(|| "attachment".to_owned()),
            size_bytes: part.len(),
        })
        .collect();
    Ok(EmailData {
        subject: message.subject().map(str::to_owned),
        from: people(message.from()).into_iter().next(),
        to: people(message.to()),
        cc: people(message.cc()),
        date: message.date().map(|date| date.to_rfc3339()),
        body: normalize_body(&body),
        attachments,
    })
}

fn read_msg(bytes: &[u8]) -> Result<EmailData> {
    let mut compound = cfb::CompoundFile::open(Cursor::new(bytes))
        .context("this does not look like an Outlook .msg file")?;

    let subject = msg_text(&mut compound, 0x0037);
    let sender_name = msg_text(&mut compound, 0x0C1A);
    let sender_address =
        msg_text(&mut compound, 0x5D01).or_else(|| msg_text(&mut compound, 0x0C1F));
    let to = msg_text(&mut compound, 0x0E04);
    let cc = msg_text(&mut compound, 0x0E03);
    let mut body = msg_text(&mut compound, 0x1000).unwrap_or_default();
    if body.trim().is_empty() {
        if let Some(html) = msg_stream(&mut compound, "/__substg1.0_10130102") {
            body = strip_html(&String::from_utf8_lossy(&html));
        }
    }
    let headers = msg_text(&mut compound, 0x007D);
    let properties = msg_stream(&mut compound, "/__properties_version1.0");

    let from = match (sender_name, sender_address) {
        (Some(name), Some(address)) if !name.is_empty() && name != address => {
            Some(format!("{name} <{address}>"))
        }
        (_, Some(address)) if !address.is_empty() => Some(address),
        (Some(name), _) if !name.is_empty() => Some(name),
        _ => None,
    };
    let date = headers
        .as_deref()
        .and_then(|headers| {
            headers
                .lines()
                .find_map(|line| line.strip_prefix("Date:").map(|value| value.trim().to_owned()))
        })
        .or_else(|| properties.as_deref().and_then(msg_submit_time));

    let mut attachments = Vec::new();
    let names = compound
        .read_storage("/")
        .map(|entries| {
            entries
                .filter(|entry| entry.is_storage())
                .map(|entry| entry.name().to_owned())
                .filter(|name| name.starts_with("__attach_version1.0_"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for storage in names {
        let name = ["3707001F", "3704001F"]
            .iter()
            .find_map(|id| msg_stream(&mut compound, &format!("/{storage}/__substg1.0_{id}")))
            .map(|data| utf16_text(&data))
            .unwrap_or_else(|| "attachment".to_owned());
        let size_bytes = msg_stream(&mut compound, &format!("/{storage}/__substg1.0_37010102"))
            .map(|data| data.len())
            .unwrap_or(0);
        attachments.push(EmailAttachmentInfo { name, size_bytes });
    }

    let split = |value: Option<String>| -> Vec<String> {
        value
            .map(|value| {
                value
                    .split(';')
                    .map(|entry| entry.trim().to_owned())
                    .filter(|entry| !entry.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok(EmailData {
        subject: subject.filter(|value| !value.is_empty()),
        from,
        to: split(to),
        cc: split(cc),
        date,
        body: normalize_body(&body),
        attachments,
    })
}

type Msg<'a> = cfb::CompoundFile<Cursor<&'a [u8]>>;

fn msg_stream(compound: &mut Msg, path: &str) -> Option<Vec<u8>> {
    let stream = compound.open_stream(path).ok()?;
    let mut data = Vec::new();
    stream.take(MAX_STREAM_BYTES).read_to_end(&mut data).ok()?;
    Some(data)
}

fn utf16_text(data: &[u8]) -> String {
    let units = data
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

/// A string property: UTF-16 (type 001F) or single-byte (type 001E).
fn msg_text(compound: &mut Msg, id: u16) -> Option<String> {
    if let Some(data) = msg_stream(compound, &format!("/__substg1.0_{id:04X}001F")) {
        return Some(utf16_text(&data));
    }
    msg_stream(compound, &format!("/__substg1.0_{id:04X}001E"))
        .map(|data| data.iter().map(|byte| *byte as char).collect())
}

/// PidTagClientSubmitTime from the top-level property stream, as ISO text.
fn msg_submit_time(properties: &[u8]) -> Option<String> {
    let mut offset = 32;
    while offset + 16 <= properties.len() {
        let tag = u32::from_le_bytes(properties[offset..offset + 4].try_into().ok()?);
        if tag == 0x0039_0040 {
            let filetime = u64::from_le_bytes(properties[offset + 8..offset + 16].try_into().ok()?);
            let unix = (filetime / 10_000_000).checked_sub(11_644_473_600)? as i64;
            let (year, month, day) = civil_from_days(unix.div_euclid(86_400));
            let seconds = unix.rem_euclid(86_400);
            return Some(format!(
                "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
                seconds / 3600,
                (seconds % 3600) / 60
            ));
        }
        offset += 16;
    }
    None
}

fn strip_html(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn normalize_body(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

fn email_to_text(email: &EmailData) -> String {
    let mut text = String::new();
    if let Some(subject) = &email.subject {
        text.push_str(&format!("Subject: {subject}\n"));
    }
    if let Some(from) = &email.from {
        text.push_str(&format!("From: {from}\n"));
    }
    if !email.to.is_empty() {
        text.push_str(&format!("To: {}\n", email.to.join(", ")));
    }
    if !email.cc.is_empty() {
        text.push_str(&format!("Cc: {}\n", email.cc.join(", ")));
    }
    if let Some(date) = &email.date {
        text.push_str(&format!("Date: {date}\n"));
    }
    if !email.attachments.is_empty() {
        let names = email
            .attachments
            .iter()
            .map(|attachment| attachment.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        text.push_str(&format!("Attachments: {names}\n"));
    }
    text.push('\n');
    text.push_str(&email.body);
    text.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ZIP archive of stored (uncompressed) entries.
    pub fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut central = Vec::new();
        for (name, data) in entries {
            let offset = bytes.len() as u32;
            bytes.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
            bytes.extend_from_slice(&20_u16.to_le_bytes());
            bytes.extend_from_slice(&[0; 8]);
            bytes.extend_from_slice(&0_u32.to_le_bytes());
            bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&0_u16.to_le_bytes());
            bytes.extend_from_slice(name.as_bytes());
            bytes.extend_from_slice(data);

            central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&[0; 8]);
            central.extend_from_slice(&0_u32.to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0; 12]);
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let central_offset = bytes.len() as u32;
        bytes.extend_from_slice(&central);
        bytes.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(central.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&central_offset.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes
    }

    fn sample_xlsx() -> Vec<u8> {
        stored_zip(&[
            (
                "xl/workbook.xml",
                br#"<workbook xmlns:r="r"><sheets><sheet name="Budget" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                br#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#,
            ),
            (
                "xl/sharedStrings.xml",
                br#"<sst><si><t>Item</t></si><si><t>R&amp;D</t></si></sst>"#,
            ),
            (
                "xl/styles.xml",
                br#"<styleSheet><cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="14"/></cellXfs></styleSheet>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                br#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="inlineStr"><is><t>Due</t></is></c></row><row r="2"><c r="A2" t="s"><v>1</v></c><c r="B2" s="1"><v>45000</v></c><c r="C2"><v>12.5</v></c></row></sheetData></worksheet>"#,
            ),
        ])
    }

    #[test]
    fn reads_xlsx_cells_dates_and_entities() {
        let sheets = read_xlsx(&sample_xlsx()).unwrap();
        assert_eq!(sheets.len(), 1);
        assert_eq!(sheets[0].name, "Budget");
        assert_eq!(sheets[0].rows[0], vec!["Item", "Due"]);
        assert_eq!(sheets[0].rows[1], vec!["R&D", "2023-03-15", "12.5"]);

        let text = extract_document_text(Path::new("budget.xlsx"), &sample_xlsx())
            .unwrap()
            .unwrap();
        assert!(text.contains("Sheet: Budget"));
        assert!(text.contains("R&D | 2023-03-15 | 12.5"));
    }

    #[test]
    fn reads_pptx_titles_text_and_notes() {
        let slide = br#"<p:sld><p:cSld><p:spTree>
            <p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Quarterly plan</a:t></a:r></a:p></p:txBody></p:sp>
            <p:sp><p:nvSpPr><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Ship the </a:t></a:r><a:r><a:t>pilot</a:t></a:r></a:p></p:txBody></p:sp>
            <p:sp><p:nvSpPr><p:nvPr><p:ph type="sldNum"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>1</a:t></a:r></a:p></p:txBody></p:sp>
        </p:spTree></p:cSld></p:sld>"#;
        let notes = br#"<p:notes><p:cSld><p:spTree>
            <p:sp><p:nvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr></p:sp>
            <p:sp><p:nvSpPr><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Mention the budget</a:t></a:r></a:p></p:txBody></p:sp>
            <p:sp><p:nvSpPr><p:nvPr><p:ph type="sldNum"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>1</a:t></a:r></a:p></p:txBody></p:sp>
        </p:spTree></p:cSld></p:notes>"#;
        let bytes = stored_zip(&[
            ("ppt/slides/slide1.xml", slide),
            ("ppt/notesSlides/notesSlide1.xml", notes),
        ]);
        let slides = read_slides(&bytes).unwrap();
        assert_eq!(slides.len(), 1);
        assert_eq!(slides[0].title.as_deref(), Some("Quarterly plan"));
        assert_eq!(slides[0].text, vec!["Ship the pilot"]);
        assert_eq!(slides[0].notes.as_deref(), Some("Mention the budget"));
    }

    #[test]
    fn reads_eml_headers_body_and_attachments() {
        let eml = b"From: Ada Lovelace <ada@example.com>\r\nTo: bob@example.com\r\nSubject: =?UTF-8?B?UXVhcnRlcmx5IHBsYW4=?=\r\nDate: Tue, 14 Mar 2023 10:00:00 +0000\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nPlease review the plan.\r\n";
        let preview = document_preview(Path::new("plan.eml"), eml);
        match preview {
            DocumentPreview::Email {
                subject,
                from,
                to,
                body,
                ..
            } => {
                assert_eq!(subject.as_deref(), Some("Quarterly plan"));
                assert_eq!(from.as_deref(), Some("Ada Lovelace <ada@example.com>"));
                assert_eq!(to, vec!["bob@example.com"]);
                assert_eq!(body, "Please review the plan.");
            }
            other => panic!("unexpected preview: {other:?}"),
        }
    }

    #[test]
    fn recovers_utf16_text_from_onenote_bytes() {
        let mut bytes = vec![0xde, 0xad, 0xbe, 0xef];
        for unit in "Meeting notes for the migration".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes.extend_from_slice(&[0, 0, 0xff, 0x01]);
        assert_eq!(onenote_text(&bytes), "Meeting notes for the migration");
    }

    #[test]
    fn unreadable_documents_fall_back_to_unavailable() {
        let preview = document_preview(Path::new("broken.xlsx"), b"not a zip");
        assert!(matches!(preview, DocumentPreview::Unavailable { .. }));
        let empty_pdf = extract_document_text(Path::new("scan.pdf"), b"%PDF-1.4\n%%EOF").unwrap();
        assert_eq!(empty_pdf.as_deref(), Some(""));
    }

    fn sample_pdf() -> Vec<u8> {
        let objects = [
            "<</Type/Catalog/Pages 2 0 R>>".to_owned(),
            "<</Type/Pages/Kids[3 0 R]/Count 1>>".to_owned(),
            "<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>".to_owned(),
            {
                let content = "BT /F1 12 Tf 20 100 Td (Hello PDF world) Tj ET";
                format!("<</Length {}>>\nstream\n{content}\nendstream", content.len())
            },
            "<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>".to_owned(),
        ];
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
        }
        let xref = pdf.len();
        pdf.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
        for offset in offsets {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str(&format!(
            "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        ));
        pdf.into_bytes()
    }

    #[test]
    fn reads_pdf_text_and_page_count() {
        let pdf = sample_pdf();
        let text = extract_document_text(Path::new("brief.pdf"), &pdf).unwrap().unwrap();
        assert!(text.contains("Hello PDF world"), "got: {text:?}");
        match document_preview(Path::new("brief.pdf"), &pdf) {
            DocumentPreview::Pdf { page_count } => assert_eq!(page_count, Some(1)),
            other => panic!("unexpected preview: {other:?}"),
        }
    }

    #[test]
    fn reads_outlook_msg_properties_and_attachments() {
        fn utf16(value: &str) -> Vec<u8> {
            value.encode_utf16().flat_map(u16::to_le_bytes).collect()
        }
        fn write(compound: &mut cfb::CompoundFile<Cursor<Vec<u8>>>, path: &str, data: &[u8]) {
            use std::io::Write;
            compound.create_stream(path).unwrap().write_all(data).unwrap();
        }
        let mut compound = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        write(&mut compound, "/__substg1.0_0037001F", &utf16("Budget approval"));
        write(&mut compound, "/__substg1.0_0C1A001F", &utf16("Ada Lovelace"));
        write(&mut compound, "/__substg1.0_5D01001F", &utf16("ada@example.com"));
        write(&mut compound, "/__substg1.0_0E04001F", &utf16("Bob; Carol"));
        write(&mut compound, "/__substg1.0_1000001F", &utf16("Numbers attached.
Thanks"));
        compound.create_storage("/__attach_version1.0_#00000000").unwrap();
        write(&mut compound, "/__attach_version1.0_#00000000/__substg1.0_3707001F", &utf16("budget.xlsx"));
        write(&mut compound, "/__attach_version1.0_#00000000/__substg1.0_37010102", &[1, 2, 3, 4]);
        let bytes = compound.into_inner().into_inner();

        match document_preview(Path::new("budget.msg"), &bytes) {
            DocumentPreview::Email {
                subject,
                from,
                to,
                body,
                attachments,
                ..
            } => {
                assert_eq!(subject.as_deref(), Some("Budget approval"));
                assert_eq!(from.as_deref(), Some("Ada Lovelace <ada@example.com>"));
                assert_eq!(to, vec!["Bob", "Carol"]);
                assert_eq!(body, "Numbers attached.\nThanks");
                assert_eq!(attachments.len(), 1);
                assert_eq!(attachments[0].name, "budget.xlsx");
                assert_eq!(attachments[0].size_bytes, 4);
            }
            other => panic!("unexpected preview: {other:?}"),
        }
    }

    #[test]
    fn excel_serial_dates_convert() {
        assert_eq!(excel_serial_text(45000.0), "2023-03-15");
        assert_eq!(excel_serial_text(45000.5), "2023-03-15 12:00");
    }
}
