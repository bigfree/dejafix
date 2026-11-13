use crate::types::Random;

pub const EDGE_LOG: &str = concat!(
    "\u{feff}",
    r#"{"files": ["src/a.py"], "id": "evt_0123456789abcdef0123", "issue_id": "0001", "location": "src/a.py:3", "summary": "Try: na\u00efve fa\u00e7ade \ud83d\ude80", "timestamp": "2026-05-12T21:07:46Z", "type": "issue", "x_fixture_extra": {"kept": true, "n": 3}}"#,
    "\n",
    r#"{"type":"note","summary":"  padded  ","timestamp":"2026-05-12 21:07:46 -0600","files":null,"auto_captured":0,"issue_id":false,"location":"","command":0.0,"unknown":null}"#,
    "\n",
    r#"{"type": "note", "summary": "hand-written note", "timestamp": "2026-10-01T08:00:00Z"}"#,
    "\r\n",
    r#"{"id": "", "type": "note", "summary": "x", "timestamp": "2026-10-01T08:00:00Z", "x": [1.5, 1e5, -0.0, 12345678901234567890, {"b": "\u00e9", "a": 1}]}"#,
    "\n",
    r#"{"id": "a", "type": "note", "summary": "dup", "id": "b", "timestamp": "2026-10-01T08:00:00Z"}"#,
    "\n",
    r#"{"id": "evt_odd", "type": "note", "summary": "odd", "timestamp": "2026-10-01T08:00:00Z", "issue_id": 7, "location": {"a": 1}, "notes": [1], "command": true}"#,
    "\n",
    "\n",
    "   \t \n",
    "{broken\n",
    "[1, 2]\n",
    r#"{"summary": "no type"}"#,
    "\n",
    r#"{"type": "bug", "summary": "x"}"#,
    "\n",
    r#"{"type": "note", "summary": " \u001f "}"#,
    "\n",
    r#"{"type": "note", "summary": 5}"#,
    "\n",
    r#"{"type": "note", "summary": "x", "outcome": "won"}"#,
    "\n",
    r#"{"type": "note", "summary": "x", "outcome": ""}"#,
    "\n",
    r#"{"type": "note", "summary": "x", "capture_source": "x"}"#,
    "\n",
    r#"{"type": "note", "summary": "x", "capture_confidence": 1}"#,
    "\n",
    r#"{"type": "note", "summary": "x", "timestamp": 5}"#,
    "\n",
    r#"{"type": "note", "summary": "x", "timestamp": "0001-01-01T00:30:00+01:00"}"#,
    "\n",
    r#"{"type": "note", "summary": "naive", "timestamp": "2026-03-08T02:30:00"}"#,
    "\n",
    r#"{"type": "note", "summary": "verbatim", "timestamp": "2026-05-12T21:07:46z"}"#,
    "\n",
    r#"{"type": "note", "summary": "week", "timestamp": "2026-W20-2"}"#,
    "\n",
    r#"{"type": "note", "summary": "falsy files", "timestamp": "2026-01-01T00:00:00Z", "files": {}, "auto_captured": "yes", "capture_source": null}"#,
    "\n",
    r#"{"type": "note", "summary": "empty file", "timestamp": "2026-01-01T00:00:00Z", "files": [""], "location": "  "}"#,
    "\n",
    r#"{"type": "issue", "issue_id": "0002", "summary": "win", "timestamp": "2026-01-01T00:00:00Z", "location": "C:\\work\\Report.cs:12"}"#,
    "\n",
    r#"{"type": "attempt", "issue_id": "0002", "outcome": "failed", "summary": "inherits", "timestamp": "2026-01-01T00:00:01Z"}"#,
    "\u{2028}",
    r#"{"type": "hypothesis", "issue_id": "0002", "summary": "inherits too", "timestamp": "2026-01-01T00:00:02Z"}"#,
    "\u{85}",
    r#"{"type": "fix", "issue_id": "0003", "summary": "fix before issue", "timestamp": "2026-01-01T00:00:03Z"}"#,
    "\n",
    r#"{"type": "issue", "issue_id": "0003", "summary": "closed early", "timestamp": "2026-01-01T00:00:04Z", "files": ["./src/b.py", "src\\c.py", "/elsewhere/d.py"]}"#,
    "\n",
    r#"{"type": "decision", "id": "evt_old", "summary": "old", "timestamp": "2026-01-01T00:00:05Z", "location": "src/a.py"}"#,
    "\n",
    r#"{"type": "note", "id": "evt_new", "summary": "retires by note", "supersedes": "evt_old", "timestamp": "2026-01-01T00:00:06Z"}"#,
    "\n",
    r#"{"type": "decision", "summary": "retires nothing", "supersedes": "", "timestamp": "2026-01-01T00:00:07Z", "location": "AuthService.validate"}"#,
    "\n",
    r#"{"type": "issue", "issue_id": "", "summary": "empty id", "timestamp": "2026-01-01T00:00:08Z", "location": "x.py:1:2:3"}"#,
    "\n",
    r#"{"type": "issue", "summary": "no id", "timestamp": "2026-01-01T00:00:09Z", "location": ".dejafix/summary.md:3"}"#,
    "\n",
);

impl Random {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound.max(1)).unwrap_or(u64::MAX);
        usize::try_from(self.next() % bound).unwrap_or(0)
    }

    pub fn pick<'p>(&mut self, items: &[&'p str]) -> &'p str {
        items
            .get(self.below(items.len()))
            .copied()
            .unwrap_or_default()
    }

    pub fn word(&mut self, alphabet: &[&str], max: usize) -> String {
        (0..self.below(max + 1))
            .map(|_| self.pick(alphabet))
            .collect()
    }
}

const LOCATION_ALPHABET: &[&str] = &[
    "/", "\\", ".", ":", "#", "'", "\"", " ", "\t", "\n", "0", "1", "9", "a", "b", "p", "y", "s",
    "r", "C", "é", "١", "\u{1c}", "\u{85}", "\u{2028}", "K", "\u{212a}", "t",
];

const PATH_ALPHABET: &[&str] = &[
    "/", "\\", ".", "..", "a", "b", ":", "C", ".dejafix", "//", " ",
];

const STAMP_ALPHABET: &[&str] = &[
    "0", "1", "2", "5", "9", "-", ":", "T", " ", "Z", "+", ".", ",", "W", "\t", "z", "é",
];

pub fn locations() -> Vec<String> {
    let fixed = [
        "src/foo.py",
        "src/foo.py:42",
        "src/foo.py:42:7",
        "foo.py",
        "C:\\src\\foo.py:42",
        "D:/repo/app.tsx:9",
        "src\\windows\\rel.py:3",
        "AuthService.validate",
        "SomeClass",
        "",
        "   ",
        "a.py:1:2:3",
        "src/a.py:1\n'",
        "a.py:1\n'",
        ":12",
        "C:\\:12",
        "x.py:١٢",
        "C:",
        "v1.2",
        "a.\u{212a}t",
        "\"'src/a.py:3'\"",
    ];
    let mut random = Random::new(0x10ca_7104);
    fixed
        .iter()
        .map(|text| (*text).to_owned())
        .chain((0..500).map(|_| random.word(LOCATION_ALPHABET, 16)))
        .collect()
}

pub fn issue_ids() -> Vec<String> {
    let fixed = [
        "1", "#1", "0001", "##12", " 7 ", "12345", "abc", "# 1", "١", "#", "  ", "²", "",
    ];
    let mut random = Random::new(0x155e);
    fixed
        .iter()
        .map(|text| (*text).to_owned())
        .chain(
            (0..300)
                .map(|_| random.word(&["#", " ", "\t", "0", "1", "7", "a", "١", "²", "\u{1f}"], 6)),
        )
        .collect()
}

pub fn timestamps() -> Vec<String> {
    let dates = [
        "2026-05-12",
        "20260512",
        "2026-W20",
        "2026-W20-2",
        "2026W202",
        "2024-02-29",
        "2026-02-29",
        "0001-01-01",
        "9999-12-31",
        "2026-5-1",
        "2026-11-01",
        "2026-03-08",
    ];
    let separators = ["T", " ", "  ", "x", "é", "\t", ""];
    let times = [
        "21:07:46",
        "21:07",
        "21",
        "2107",
        "210746",
        "21:07:46.5",
        "21:07:46,123456789",
        "24:00",
        "24:00:01",
        "1:2:3",
        "02:30:00",
        "01:30:00",
        "21:07:60",
        "",
    ];
    let zones = [
        "",
        "Z",
        "z",
        "+00:00",
        "+02:00",
        "-0600",
        "+02",
        "+02:00:30",
        "-02:00:30.5",
        "+24:00",
        "+0200:30",
        " +0000",
        " -0600",
        " Z",
        "-0",
        "+23:59:59.999999",
    ];
    let mut random = Random::new(0x7157);
    let mut out: Vec<String> = Vec::new();
    for _ in 0..1_500 {
        out.push(format!(
            "{}{}{}{}",
            random.pick(&dates),
            random.pick(&separators),
            random.pick(&times),
            random.pick(&zones)
        ));
    }
    out.extend((0..300).map(|_| random.word(STAMP_ALPHABET, 24)));
    out.retain(|text| !text.is_empty());
    out
}

pub fn paths(root: &str) -> Vec<String> {
    let mut random = Random::new(0x9a7);
    let fixed = [
        "src/a.py",
        "./src/a.py",
        "src//a.py",
        "src\\a.py",
        "/elsewhere/a.py",
        ".",
        "..",
        "../.dejafix/summary.md",
        ".dejafix/x",
        "sub/.dejafix/x",
        "C:\\work\\x",
        "D:\\repo\\.dejafix\\x",
        "",
    ];
    fixed
        .iter()
        .map(|text| (*text).to_owned())
        .chain([
            format!("{root}/src/a.py"),
            format!("{root}/.dejafix/x"),
            root.to_owned(),
            format!("{root}/../x"),
        ])
        .chain((0..400).map(|_| {
            let tail = random.word(PATH_ALPHABET, 8);
            if random.below(3) == 0 {
                format!("{root}/{tail}")
            } else {
                tail
            }
        }))
        .collect()
}
