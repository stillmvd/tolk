use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::{OcrEngine, OcrResult};
use windows::Security::Cryptography::CryptographicBuffer;

#[derive(Debug, PartialEq)]
pub enum OcrError {
    NoLanguage,
    Empty,
    Failed(String),
}

impl OcrError {
    pub fn message(&self) -> String {
        match self {
            OcrError::NoLanguage => "Нет распознавания текста — добавь его: Параметры → Время и язык → Язык и регион → English или Русский → Параметры языка".into(),
            OcrError::Empty => "Текст в области не найден".into(),
            OcrError::Failed(e) => format!("Не получилось распознать текст: {e}"),
        }
    }
}

pub struct Line {
    pub text: String,
    pub left: f32,
    pub top: f32,
    pub bottom: f32,
}

pub fn max_side() -> u32 {
    OcrEngine::MaxImageDimension().unwrap_or(2600)
}

pub fn recognize(bgra: &[u8], width: u32, height: u32) -> Result<String, OcrError> {
    let fail = |e: windows::core::Error| OcrError::Failed(e.message());
    let buf = CryptographicBuffer::CreateFromByteArray(bgra).map_err(fail)?;
    let bmp = SoftwareBitmap::CreateCopyFromBuffer(&buf, BitmapPixelFormat::Bgra8, width as i32, height as i32).map_err(fail)?;
    let langs: Vec<String> = OcrEngine::AvailableRecognizerLanguages().map_err(fail)?.into_iter().filter_map(|l| l.LanguageTag().ok()).map(|t| t.to_string()).collect();
    let run = |prefix: &str| -> Result<Option<String>, OcrError> {
        let Some(tag) = langs.iter().find(|t| t.to_lowercase().starts_with(prefix)) else { return Ok(None) };
        let lang = Language::CreateLanguage(&HSTRING::from(tag.as_str())).map_err(fail)?;
        let engine = OcrEngine::TryCreateFromLanguage(&lang).map_err(fail)?;
        let result = engine.RecognizeAsync(&bmp).map_err(fail)?.join().map_err(fail)?;
        Ok(Some(layout(&lines(&result).map_err(fail)?)))
    };
    let ru = run("ru")?;
    let en = run("en")?;
    if ru.is_none() && en.is_none() {
        return Err(OcrError::NoLanguage);
    }
    let text = choose(ru, en);
    if text.trim().is_empty() {
        return Err(OcrError::Empty);
    }
    Ok(text)
}

fn lines(result: &OcrResult) -> windows::core::Result<Vec<Line>> {
    let mut out = Vec::new();
    for line in result.Lines()? {
        let (mut left, mut top, mut bottom) = (f32::MAX, f32::MAX, f32::MIN);
        for word in line.Words()? {
            let r = word.BoundingRect()?;
            left = left.min(r.X);
            top = top.min(r.Y);
            bottom = bottom.max(r.Y + r.Height);
        }
        if top <= bottom {
            out.push(Line { text: line.Text()?.to_string(), left, top, bottom });
        }
    }
    Ok(out)
}

pub fn choose(ru: Option<String>, en: Option<String>) -> String {
    match (ru, en) {
        (Some(ru), _) if cyrillic_share(&ru) > 0.5 => ru,
        (_, Some(en)) if !en.trim().is_empty() => en,
        (ru, en) => ru.or(en).unwrap_or_default(),
    }
}

fn cyrillic_share(text: &str) -> f32 {
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    if letters == 0 {
        return 0.0;
    }
    let cyr = text.chars().filter(|c| matches!(c, '\u{0400}'..='\u{04FF}')).count();
    cyr as f32 / letters as f32
}

pub fn layout(lines: &[Line]) -> String {
    let mut heights: Vec<f32> = lines.iter().map(|l| l.bottom - l.top).collect();
    heights.sort_by(|a, b| a.total_cmp(b));
    let typical = heights.get(heights.len() / 2).copied().unwrap_or(0.0);
    let mut out = String::new();
    let mut prev: Option<&Line> = None;
    for line in lines {
        let text = line.text.trim();
        if text.is_empty() {
            continue;
        }
        if let Some(p) = prev {
            let gap = line.top - p.bottom;
            if gap > typical * 0.8 || line.left - p.left > typical * 1.5 {
                out.push('\n');
            } else if out.ends_with('-') && out.chars().rev().nth(1).is_some_and(char::is_alphabetic) && text.starts_with(char::is_lowercase) {
                out.pop();
            } else {
                out.push(' ');
            }
        }
        out.push_str(text);
        prev = Some(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, left: f32, top: f32) -> Line {
        Line { text: text.into(), left, top, bottom: top + 20.0 }
    }

    #[test]
    fn joins_lines_of_a_paragraph() {
        let l = [line("Good morning,", 0.0, 0.0), line("friends", 0.0, 26.0)];
        assert_eq!(layout(&l), "Good morning, friends");
    }

    #[test]
    fn splits_paragraphs_by_gap() {
        let l = [line("Hello", 0.0, 0.0), line("open the lock", 0.0, 60.0)];
        assert_eq!(layout(&l), "Hello\nopen the lock");
    }

    #[test]
    fn splits_paragraph_by_indent() {
        let l = [line("First one.", 0.0, 0.0), line("Second one.", 40.0, 26.0)];
        assert_eq!(layout(&l), "First one.\nSecond one.");
    }

    #[test]
    fn glues_hyphenated_word() {
        let l = [line("transla-", 0.0, 0.0), line("tion works", 0.0, 26.0)];
        assert_eq!(layout(&l), "translation works");
    }

    #[test]
    fn picks_russian_when_cyrillic() {
        assert_eq!(choose(Some("Доброе утро".into()), Some("Ao6poe ytpo".into())), "Доброе утро");
        assert_eq!(choose(Some("Good morning".into()), Some("Good morning".into())), "Good morning");
        assert_eq!(choose(None, Some("Hello".into())), "Hello");
        assert_eq!(choose(Some("Привет".into()), None), "Привет");
    }
}
