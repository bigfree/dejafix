use std::borrow::Cow;

use super::types::{MAX_STEM_BYTES, Word};

const EXCEPTIONS: [(&str, &str); 15] = [
    ("andes", "andes"),
    ("atlas", "atlas"),
    ("bias", "bias"),
    ("cosmos", "cosmos"),
    ("early", "earli"),
    ("gently", "gentl"),
    ("howe", "howe"),
    ("idly", "idl"),
    ("news", "news"),
    ("only", "onli"),
    ("singly", "singl"),
    ("skies", "sky"),
    ("skis", "ski"),
    ("sky", "sky"),
    ("ugly", "ugli"),
];

const R1_PREFIXES: [&[u8]; 9] = [
    b"gener", b"commun", b"arsen", b"past", b"univers", b"later", b"emerg", b"organ", b"inter",
];

const DOUBLES: [&[u8]; 9] = [
    b"bb", b"dd", b"ff", b"gg", b"mm", b"nn", b"pp", b"rr", b"tt",
];

const ING_KEPT: [&[u8]; 6] = [b"inn", b"out", b"cann", b"herr", b"earr", b"even"];

const EED_KEPT: [&[u8]; 3] = [b"proc", b"exc", b"succ"];

const STEP_2: [(&[u8], &[u8]); 26] = [
    (b"ational", b"ate"),
    (b"fulness", b"ful"),
    (b"iveness", b"ive"),
    (b"ization", b"ize"),
    (b"ousness", b"ous"),
    (b"biliti", b"ble"),
    (b"lessli", b"less"),
    (b"tional", b"tion"),
    (b"alism", b"al"),
    (b"aliti", b"al"),
    (b"ation", b"ate"),
    (b"entli", b"ent"),
    (b"fulli", b"ful"),
    (b"iviti", b"ive"),
    (b"ogist", b"og"),
    (b"ousli", b"ous"),
    (b"abli", b"able"),
    (b"alli", b"al"),
    (b"anci", b"ance"),
    (b"ator", b"ate"),
    (b"enci", b"ence"),
    (b"izer", b"ize"),
    (b"bli", b"ble"),
    (b"ogi", b"og"),
    (b"li", b""),
    (b"", b""),
];

const STEP_3: [(&[u8], &[u8]); 10] = [
    (b"ational", b"ate"),
    (b"tional", b"tion"),
    (b"alize", b"al"),
    (b"ative", b""),
    (b"icate", b"ic"),
    (b"iciti", b"ic"),
    (b"ical", b"ic"),
    (b"ness", b""),
    (b"ful", b""),
    (b"", b""),
];

const STEP_4: [&[u8]; 19] = [
    b"ement", b"able", b"ance", b"ence", b"ible", b"ment", b"ant", b"ate", b"ent", b"ion", b"ism",
    b"iti", b"ive", b"ize", b"ous", b"al", b"er", b"ic", b"",
];

fn is_vowel(byte: u8) -> bool {
    matches!(byte, b'a' | b'e' | b'i' | b'o' | b'u' | b'y')
}

fn is_li_ending(byte: u8) -> bool {
    matches!(
        byte,
        b'c' | b'd' | b'e' | b'g' | b'h' | b'k' | b'm' | b'n' | b'r' | b't'
    )
}

fn past_vowel_then_consonant(bytes: &[u8], from: usize) -> Option<usize> {
    let rest = bytes.get(from..)?;
    let vowel = rest.iter().position(|byte| is_vowel(*byte))?;
    let consonant = rest
        .iter()
        .skip(vowel + 1)
        .position(|byte| !is_vowel(*byte))?;
    Some(from + vowel + 1 + consonant + 1)
}

impl Word {
    fn new(bytes: &[u8]) -> Option<Self> {
        let mut word = Self {
            bytes: [0; MAX_STEM_BYTES],
            len: bytes.len(),
            p1: bytes.len(),
            p2: bytes.len(),
        };
        word.bytes.get_mut(..bytes.len())?.copy_from_slice(bytes);
        Some(word)
    }

    fn text(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or_default()
    }

    fn at(&self, index: usize) -> Option<u8> {
        self.text().get(index).copied()
    }

    fn ends_with(&self, suffix: &[u8]) -> bool {
        self.text().ends_with(suffix)
    }

    fn before(&self, suffix_len: usize) -> &[u8] {
        self.text()
            .get(..self.len.saturating_sub(suffix_len))
            .unwrap_or_default()
    }

    fn replace(&mut self, suffix_len: usize, with: &[u8]) {
        let start = self.len.saturating_sub(suffix_len);
        if let Some(slot) = self.bytes.get_mut(start..start + with.len()) {
            slot.copy_from_slice(with);
            self.len = start + with.len();
        }
    }

    fn longest<'s, T>(&self, table: &'s [(&'s [u8], T)]) -> Option<&'s (&'s [u8], T)> {
        table.iter().find(|(suffix, _)| self.ends_with(suffix))
    }

    fn short_syllable_at(&self, end: usize) -> bool {
        let text = self.text().get(..end).unwrap_or_default();
        match text {
            [.., before, vowel, last] => {
                (!is_vowel(*before)
                    && is_vowel(*vowel)
                    && !is_vowel(*last)
                    && !matches!(last, b'w' | b'x' | b'Y'))
                    || text.ends_with(b"past")
            }
            [vowel, last] => is_vowel(*vowel) && !is_vowel(*last),
            _ => false,
        }
    }

    fn prelude(&mut self) {
        if let Some(first) = self.bytes.first_mut()
            && *first == b'y'
        {
            *first = b'Y';
        }
        let mut previous_vowel = false;
        for byte in self.bytes.iter_mut().take(self.len) {
            if *byte == b'y' && previous_vowel {
                *byte = b'Y';
            }
            previous_vowel = is_vowel(*byte);
        }
    }

    fn mark_regions(&mut self) {
        let text = self.text();
        let p1 = R1_PREFIXES
            .iter()
            .find(|prefix| text.starts_with(prefix))
            .map(|prefix| prefix.len())
            .or_else(|| past_vowel_then_consonant(text, 0));
        if let Some(p1) = p1 {
            let p2 = past_vowel_then_consonant(text, p1).unwrap_or(self.len);
            self.p1 = p1;
            self.p2 = p2;
        }
    }

    fn step_0(&mut self) {
        if let Some(suffix) = [&b"'s'"[..], b"'s", b"'"]
            .into_iter()
            .find(|suffix| self.ends_with(suffix))
        {
            self.replace(suffix.len(), b"");
        }
    }

    fn step_1a(&mut self) {
        if self.ends_with(b"sses") {
            self.replace(4, b"ss");
        } else if self.ends_with(b"ied") || self.ends_with(b"ies") {
            let with: &[u8] = if self.len > 4 { b"i" } else { b"ie" };
            self.replace(3, with);
        } else if self.ends_with(b"s")
            && !self.ends_with(b"us")
            && !self.ends_with(b"ss")
            && self.before(2).iter().any(|byte| is_vowel(*byte))
        {
            self.replace(1, b"");
        }
    }

    fn step_1b(&mut self) {
        let suffix_len = [&b"eedly"[..], b"ingly", b"edly", b"eed", b"ing", b"ed"]
            .into_iter()
            .find(|suffix| self.ends_with(suffix))
            .map_or(0, <[u8]>::len);
        if suffix_len == 0 {
            return;
        }
        let stem = self.before(suffix_len);
        if self.ends_with(b"eed") || self.ends_with(b"eedly") {
            if self.len - suffix_len >= self.p1 && !EED_KEPT.contains(&stem) {
                self.replace(suffix_len, b"ee");
            }
            return;
        }
        if self.ends_with(b"ing") {
            if let [first, b'y'] = stem
                && !is_vowel(*first)
            {
                self.replace(4, b"ie");
                return;
            }
            if ING_KEPT.contains(&stem) {
                return;
            }
        }
        if !stem.iter().any(|byte| is_vowel(*byte)) {
            return;
        }
        self.replace(suffix_len, b"");
        if self.ends_with(b"at") || self.ends_with(b"bl") || self.ends_with(b"iz") {
            self.replace(0, b"e");
        } else if DOUBLES.iter().any(|double| self.ends_with(double)) {
            if !matches!(self.text(), [b'a' | b'e' | b'o', _, _]) {
                self.replace(1, b"");
            }
        } else if self.p1 == self.len && self.short_syllable_at(self.len) {
            self.replace(0, b"e");
        }
    }

    fn step_1c(&mut self) {
        if self.len > 2
            && matches!(self.at(self.len - 1), Some(b'y' | b'Y'))
            && self.at(self.len - 2).is_some_and(|byte| !is_vowel(byte))
        {
            self.replace(1, b"i");
        }
    }

    fn step_2(&mut self) {
        let Some((suffix, with)) = self.longest(&STEP_2) else {
            return;
        };
        if suffix.is_empty() || self.len - suffix.len() < self.p1 {
            return;
        }
        let preceding = self.before(suffix.len()).last().copied();
        let allowed = match *suffix {
            b"ogi" => preceding == Some(b'l'),
            b"li" => preceding.is_some_and(is_li_ending),
            _ => true,
        };
        if allowed {
            self.replace(suffix.len(), with);
        }
    }

    fn step_3(&mut self) {
        let Some((suffix, with)) = self.longest(&STEP_3) else {
            return;
        };
        let start = self.len - suffix.len();
        if suffix.is_empty() || start < self.p1 || (*suffix == b"ative" && start < self.p2) {
            return;
        }
        self.replace(suffix.len(), with);
    }

    fn step_4(&mut self) {
        let Some(suffix) = STEP_4.iter().find(|suffix| self.ends_with(suffix)) else {
            return;
        };
        if suffix.is_empty() || self.len - suffix.len() < self.p2 {
            return;
        }
        if *suffix == b"ion" && !matches!(self.before(suffix.len()).last(), Some(b's' | b't')) {
            return;
        }
        self.replace(suffix.len(), b"");
    }

    fn step_5(&mut self) {
        let Some(last) = self.len.checked_sub(1) else {
            return;
        };
        match self.at(last) {
            Some(b'e') if last >= self.p2 || (last >= self.p1 && !self.short_syllable_at(last)) => {
                self.replace(1, b"");
            }
            Some(b'l') if last >= self.p2 && self.at(last.wrapping_sub(1)) == Some(b'l') => {
                self.replace(1, b"");
            }
            _ => {}
        }
    }

    fn postlude(&mut self) {
        for byte in self.bytes.iter_mut().take(self.len) {
            if *byte == b'Y' {
                *byte = b'y';
            }
        }
    }
}

#[must_use]
pub fn stem(word: &str) -> Cow<'_, str> {
    if let Ok(found) = EXCEPTIONS.binary_search_by(|(from, _)| (*from).cmp(word)) {
        return Cow::Borrowed(EXCEPTIONS.get(found).map_or(word, |(_, to)| *to));
    }
    let bytes = word.as_bytes();
    if bytes.len() < 3
        || bytes.len() > MAX_STEM_BYTES
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || *byte == b'\'')
    {
        return Cow::Borrowed(word);
    }
    let offset = usize::from(bytes.first() == Some(&b'\''));
    let Some(mut stemmed) = bytes.get(offset..).and_then(Word::new) else {
        return Cow::Borrowed(word);
    };
    stemmed.prelude();
    stemmed.mark_regions();
    stemmed.step_0();
    stemmed.step_1a();
    stemmed.step_1b();
    stemmed.step_1c();
    stemmed.step_2();
    stemmed.step_3();
    stemmed.step_4();
    stemmed.step_5();
    stemmed.postlude();
    let text = stemmed.text();
    match word.get(offset..offset + text.len()) {
        Some(same) if same.as_bytes() == text => Cow::Borrowed(same),
        _ => std::str::from_utf8(text)
            .map_or(Cow::Borrowed(word), |owned| Cow::Owned(owned.to_owned())),
    }
}
