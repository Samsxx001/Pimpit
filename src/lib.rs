//! Pure conversion engine. Dictionaries stay in the executable's read-only pages.
pub mod game_mode;
const EN: &[u8] = include_bytes!("../data/english.txt");
const TH: &[u8] = include_bytes!("../data/thai.txt");

// US QWERTY and Thai Kedmanee, including shifted punctuation.
const US: &str = "`1234567890-=qwertyuiop[]\\asdfghjkl;'zxcvbnm,./~!@#$%^&*()_+QWERTYUIOP{}|ASDFGHJKL:\"ZXCVBNM<>?";
const THAI: &str =
    "_ๅ/-ภถุึคตจขชๆไำพะัีรนยบลฃฟหกดเ้่าสวงผปแอิืทมใฝ%+๑๒๓๔ู฿๕๖๗๘๙๐\"ฎฑธํ๊ณฯญฐ,ฅฤฆฏโฌ็๋ษศซ.()ฉฮฺ์?ฒฬฦ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Thai,
}

pub fn convert(text: &str, from: Language) -> Option<String> {
    let (source, target) = match from {
        Language::English => (US, THAI),
        Language::Thai => (THAI, US),
    };
    text.chars()
        .map(|c| {
            if c == ' ' {
                return Some(' ');
            }
            source
                .chars()
                .position(|s| s == c)
                .and_then(|i| target.chars().nth(i))
        })
        .collect()
}

/// Binary search packed newline-separated UTF-8, without a heap index.
fn search(data: &[u8], word: &str, prefix: bool) -> bool {
    if word.is_empty() {
        return false;
    }
    let (mut low, mut high) = (0, data.len());
    while low < high {
        let middle = low + (high - low) / 2;
        let mut start = middle;
        while start > low && data[start - 1] != b'\n' {
            start -= 1;
        }
        let end = start
            + data[start..high]
                .iter()
                .position(|&b| b == b'\n')
                .unwrap_or(high - start);
        let line = &data[start..end];
        if prefix && line.starts_with(word.as_bytes()) {
            return true;
        }
        match line.cmp(word.as_bytes()) {
            std::cmp::Ordering::Equal => return true,
            std::cmp::Ordering::Less => low = (end + 1).min(high),
            std::cmp::Ordering::Greater => high = start,
        }
    }
    false
}

pub fn contains(data: &[u8], word: &str) -> bool {
    search(data, word, false)
}

/// A normal word can be unfinished while the user is still typing.
fn has_prefix(data: &[u8], word: &str) -> bool {
    search(data, word, true)
}

fn thai_phrase_known(text: &str) -> bool {
    if contains(TH, text) {
        return true;
    }
    let mut boundary = [0usize; 97];
    let mut count = 0;
    for (byte, _) in text.char_indices() {
        if count == 96 {
            return false;
        }
        boundary[count] = byte;
        count += 1;
    }
    boundary[count] = text.len();
    let mut reachable = [false; 97];
    reachable[0] = true;
    for start in 0..count {
        if !reachable[start] {
            continue;
        }
        // A one-letter segmentation is too ambiguous for automatic correction.
        for end in (start + 2)..=count {
            if !reachable[end] && contains(TH, &text[boundary[start]..boundary[end]]) {
                reachable[end] = true;
                if end == count {
                    return true;
                }
            }
        }
    }
    false
}

pub fn known(text: &str, language: Language) -> bool {
    match language {
        Language::English => contains(EN, &text.to_ascii_lowercase()),
        Language::Thai => contains(TH, text),
    }
}

/// Conservative: preserve recognized source words, short tokens, URLs and mixed scripts.
pub fn automatic(text: &str, from: Language) -> Option<String> {
    if text.chars().count() < 3
        || text.chars().count() > 96
        || known(text, from)
        || (from == Language::Thai && thai_phrase_known(text))
    {
        return None;
    }
    if text.contains(['@', ':', '/', '\\', '.', '_', '\n', '\t']) {
        return None;
    }
    let candidate = convert(text, from)?;
    let target = match from {
        Language::English => Language::Thai,
        Language::Thai => Language::English,
    };
    if target == Language::English && !candidate.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let recognized = match target {
        Language::English => known(&candidate, target),
        Language::Thai => thai_phrase_known(&candidate),
    };
    recognized.then_some(candidate)
}

/// No Space required. Do not replace a prefix of a valid source-language word.
/// The caller waits briefly for the final key to settle and cancels on new input.
pub fn automatic_live(text: &str, from: Language) -> Option<String> {
    let unfinished = match from {
        Language::English => has_prefix(EN, &text.to_ascii_lowercase()),
        Language::Thai => has_prefix(TH, text),
    };
    if unfinished {
        return None;
    }
    automatic(text, from)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_table() {
        assert_eq!(US.chars().count(), 94);
        assert_eq!(THAI.chars().count(), 94);
        for c in US.chars() {
            let encoded = convert(&c.to_string(), Language::English).unwrap();
            assert_eq!(convert(&encoded, Language::Thai).unwrap(), c.to_string());
        }
    }
    #[test]
    fn common_mistakes() {
        assert_eq!(
            convert("l;ylfu", Language::English).as_deref(),
            Some("สวัสดี")
        );
        assert_eq!(
            automatic("l;ylfu", Language::English).as_deref(),
            Some("สวัสดี")
        );
        assert_eq!(automatic("้ำสสน", Language::Thai).as_deref(), Some("hello"));
        assert_eq!(convert("dki", Language::English).as_deref(), Some("การ"));
        assert_eq!(automatic("dki", Language::English).as_deref(), Some("การ"));
    }
    #[test]
    fn live_correction_without_space_includes_short_words_and_thai_phrases() {
        assert_eq!(
            automatic_live("dki", Language::English).as_deref(),
            Some("การ")
        );
        assert_eq!(
            automatic_live("l;ylfu", Language::English).as_deref(),
            Some("สวัสดี")
        );
        assert_eq!(
            automatic_live("l;ylfu8iy[", Language::English).as_deref(),
            Some("สวัสดีครับ")
        );
        assert_eq!(
            automatic_live("้ำสสน", Language::Thai).as_deref(),
            Some("hello")
        );
        for word in [
            "hello",
            "computer",
            "com",
            "you",
            "the",
            "he",
            "a",
            "h",
            "hel",
            "comp",
            "www.example.com",
            "foo@bar.com",
        ] {
            assert_eq!(automatic_live(word, Language::English), None, "{word}");
        }
        assert_eq!(automatic_live("สวัสดีครับ", Language::Thai), None);
    }
    #[test]
    fn prefix_lookup_is_not_confused_with_exact_lookup() {
        let data = b"a\ncomputer\nhello\nworld\n";
        assert!(has_prefix(data, "comp"));
        assert!(!contains(data, "comp"));
        assert!(has_prefix(data, "world"));
        assert!(!has_prefix(data, "worlds"));
        assert!(!has_prefix(data, ""));
        assert!(!has_prefix(data, "z"));
    }
    #[test]
    fn preserves_valid_and_ambiguous_input() {
        for s in [
            "hello",
            "world",
            "computer",
            "rust",
            "test",
            "a",
            "abc",
            "user@example.com",
            "https://abc",
            "C:\\test",
            "1234",
            "helloไทย",
        ] {
            assert_eq!(automatic(s, Language::English), None, "{s}");
        }
        for s in ["สวัสดี", "ทดสอบ", "ภาษาไทย", "ขอบคุณ"]
        {
            assert_eq!(automatic(s, Language::Thai), None);
        }
        assert_eq!(convert("🙂", Language::Thai), None);
    }
    #[test]
    fn dictionary_search_exhaustive() {
        for data in [EN, TH] {
            let text = std::str::from_utf8(data).unwrap();
            let mut previous = "";
            for word in text.lines() {
                assert!(word > previous, "dictionary order: {word}");
                assert!(contains(data, word), "missing {word}");
                previous = word;
            }
            assert!(!contains(data, ""));
            assert!(!contains(data, "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"));
        }
        for bytes in [b"".as_slice(), b"a\n", b"a\nb\nc\n", b"a\nbb\nccc"] {
            for word in ["a", "b", "bb", "c", "ccc", "x", ""] {
                assert_eq!(
                    contains(bytes, word),
                    std::str::from_utf8(bytes)
                        .unwrap()
                        .lines()
                        .any(|w| w == word)
                );
            }
        }
    }
}
