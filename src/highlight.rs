use egui::text::LayoutJob;
use egui::Color32;

#[derive(Clone, Copy, PartialEq)]
pub enum Language {
    Json,
    Xml,
    Plain,
}

pub fn detect_language(text: &str) -> Language {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Language::Plain;
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return Language::Json;
        }
    }
    if trimmed.starts_with('<') && trimmed.contains('>') {
        return Language::Xml;
    }
    Language::Plain
}

pub fn make_layouter() -> impl FnMut(&egui::Ui, &str, f32) -> std::sync::Arc<egui::Galley> {
    move |ui: &egui::Ui, text: &str, wrap_width: f32| {
        let lang = detect_language(text);
        let job = highlight(text, lang, wrap_width);
        ui.fonts(|f| f.layout_job(job))
    }
}

fn highlight(text: &str, lang: Language, wrap_width: f32) -> LayoutJob {
    match lang {
        Language::Json => highlight_json(text, wrap_width),
        Language::Xml => highlight_xml(text, wrap_width),
        Language::Plain => plain_job(text, wrap_width),
    }
}

fn plain_job(text: &str, wrap_width: f32) -> LayoutJob {
    LayoutJob::simple(text.to_string(), egui::FontId::monospace(12.0), Color32::LIGHT_GRAY, wrap_width)
}

// ---- JSON highlighting ----

fn highlight_json(text: &str, _wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let pairs: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < pairs.len() {
        let (_, ch) = pairs[i];
        if ch.is_whitespace() {
            let start = i;
            while i < pairs.len() && pairs[i].1.is_whitespace() {
                i += 1;
            }
            let byte_start = pairs[start].0;
            let byte_end = if i < pairs.len() { pairs[i].0 } else { text.len() };
            job.append(&text[byte_start..byte_end], 0.0, text_format(Color32::LIGHT_GRAY));
        } else if ch == '"' {
            let start = i;
            i += 1;
            while i < pairs.len() {
                if pairs[i].1 == '\\' && i + 1 < pairs.len() {
                    i += 2;
                } else if pairs[i].1 == '"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            let byte_start = pairs[start].0;
            let byte_end = if i < pairs.len() { pairs[i].0 } else { text.len() };
            let s = &text[byte_start..byte_end];
            // Check if this string is a key (followed by ':')
            let after = text[byte_end..].trim_start();
            let is_key = after.starts_with(':');
            let color = if is_key { Color32::from_rgb(86, 156, 214) } else { Color32::from_rgb(78, 201, 176) };
            job.append(s, 0.0, text_format(color));
        } else if ch == ':' || ch == ',' || ch == '{' || ch == '}' || ch == '[' || ch == ']' {
            let byte_start = pairs[i].0;
            let byte_end = byte_start + ch.len_utf8();
            job.append(&text[byte_start..byte_end], 0.0, text_format(Color32::LIGHT_GRAY));
            i += 1;
        } else if ch == '-' || ch.is_ascii_digit() {
            let start = i;
            if ch == '-' { i += 1; }
            while i < pairs.len() && (pairs[i].1.is_ascii_digit() || pairs[i].1 == '.' || pairs[i].1 == 'e' || pairs[i].1 == 'E' || pairs[i].1 == '+' || pairs[i].1 == '-') {
                if (pairs[i].1 == '+' || pairs[i].1 == '-') && i > start && pairs[i-1].1 != 'e' && pairs[i-1].1 != 'E' {
                    break;
                }
                i += 1;
            }
            let byte_start = pairs[start].0;
            let byte_end = if i < pairs.len() { pairs[i].0 } else { text.len() };
            job.append(&text[byte_start..byte_end], 0.0, text_format(Color32::from_rgb(229, 192, 123)));
        } else if text[pairs[i].0..].starts_with("true") || text[pairs[i].0..].starts_with("false") {
            let end = if text[pairs[i].0..].starts_with("true") { pairs[i].0 + 4 } else { pairs[i].0 + 5 };
            job.append(&text[pairs[i].0..end], 0.0, text_format(Color32::from_rgb(86, 156, 214)));
            while i < pairs.len() && pairs[i].0 < end { i += 1; }
        } else if text[pairs[i].0..].starts_with("null") {
            let end = pairs[i].0 + 4;
            job.append(&text[pairs[i].0..end], 0.0, text_format(Color32::from_rgb(86, 156, 214)));
            while i < pairs.len() && pairs[i].0 < end { i += 1; }
        } else {
            let byte_start = pairs[i].0;
            let byte_end = byte_start + ch.len_utf8();
            job.append(&text[byte_start..byte_end], 0.0, text_format(Color32::LIGHT_GRAY));
            i += 1;
        }
    }
    job
}

// ---- XML highlighting ----

fn highlight_xml(text: &str, _wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let pairs: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < pairs.len() {
        let (byte_i, ch) = pairs[i];
        if text[byte_i..].starts_with("<!--") {
            let end = text[byte_i..].find("-->").map(|p| byte_i + p + 3).unwrap_or(text.len());
            job.append(&text[byte_i..end], 0.0, text_format(Color32::from_rgb(106, 153, 85)));
            while i < pairs.len() && pairs[i].0 < end { i += 1; }
        } else if ch == '<' {
            let start_byte = byte_i;
            let mut end_byte = byte_i;
            i += 1;
            while i < pairs.len() && pairs[i].1 != '>' {
                if text[pairs[i].0..].starts_with("/>") {
                    end_byte = pairs[i].0 + 2;
                    i += 1;
                    break;
                }
                i += 1;
            }
            if i < pairs.len() && pairs[i].1 == '>' {
                end_byte = pairs[i].0 + 1;
                i += 1;
            } else if end_byte == start_byte {
                end_byte = text.len();
            }
            job.append(&text[start_byte..end_byte], 0.0, tag_format());
        } else if ch == '&' {
            let start_byte = byte_i;
            i += 1;
            while i < pairs.len() && pairs[i].1 != ';' { i += 1; }
            let end_byte = if i < pairs.len() { pairs[i].0 + 1 } else { text.len() };
            if i < pairs.len() { i += 1; }
            job.append(&text[start_byte..end_byte], 0.0, text_format(Color32::from_rgb(86, 156, 214)));
        } else {
            let start_byte = byte_i;
            i += 1;
            while i < pairs.len() && pairs[i].1 != '<' && pairs[i].1 != '&' { i += 1; }
            let end_byte = if i < pairs.len() { pairs[i].0 } else { text.len() };
            job.append(&text[start_byte..end_byte], 0.0, text_format(Color32::LIGHT_GRAY));
        }
    }
    job
}

fn tag_format() -> egui::TextFormat {
    egui::TextFormat::simple(egui::FontId::monospace(12.0), Color32::from_rgb(86, 156, 214))
}

fn text_format(color: Color32) -> egui::TextFormat {
    egui::TextFormat::simple(egui::FontId::monospace(12.0), color)
}

// ---- Formatting ----

pub fn format_json(text: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

pub fn format_xml(text: &str) -> Result<String, String> {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut indent: i32 = 0;
    let mut in_tag = false;
    while i < chars.len() {
        if text[i..].starts_with("<!--") {
            let end = text[i..].find("-->").map(|p| i + p + 3).unwrap_or(text.len());
            for ch in text[i..end].chars() { out.push(ch); }
            out.push('\n');
            i = end;
            continue;
        }
        if chars[i] == '<' {
            if !in_tag {
                let is_close = i + 1 < chars.len() && chars[i + 1] == '/';
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                if !is_close {
                    for _ in 0..indent { out.push_str("  "); }
                } else {
                    for _ in 0..(indent - 1).max(0) { out.push_str("  "); }
                }
                if !is_close { indent += 1; } else { indent = (indent - 1).max(0); }
            }
            in_tag = true;
            out.push('<');
            i += 1;
        } else if chars[i] == '>' {
            in_tag = false;
            out.push('>');
            out.push('\n');
            i += 1;
        } else if in_tag {
            out.push(chars[i]);
            i += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    Ok(out)
}
