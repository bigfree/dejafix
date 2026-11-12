use std::borrow::Cow;

use super::casefold_table::CASEFOLD;

fn folded(character: char) -> Option<&'static str> {
    CASEFOLD
        .binary_search_by_key(&character, |(from, _)| *from)
        .ok()
        .and_then(|index| CASEFOLD.get(index))
        .map(|(_, to)| *to)
}

#[must_use]
pub fn casefold(text: &str) -> Cow<'_, str> {
    let Some((start, _)) = text
        .char_indices()
        .find(|(_, character)| folded(*character).is_some())
    else {
        return Cow::Borrowed(text);
    };
    let mut out = String::with_capacity(text.len() + 8);
    out.push_str(text.get(..start).unwrap_or_default());
    for character in text.get(start..).unwrap_or_default().chars() {
        match folded(character) {
            Some(to) => out.push_str(to),
            None => out.push(character),
        }
    }
    Cow::Owned(out)
}

#[cfg(any(test, feature = "testing"))]
#[must_use]
pub fn casefold_table() -> &'static [(char, &'static str)] {
    CASEFOLD
}
