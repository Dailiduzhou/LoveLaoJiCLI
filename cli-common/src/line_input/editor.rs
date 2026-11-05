//! Pure single-line editor: bytes in, echo bytes and a decision out. No I/O.
//! The submitted answer is exact; erase echo across wrapped rows is best-effort.

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    More,
    Submit(Option<String>),
    Eof,
}

#[derive(Clone, Copy)]
enum Escape {
    None,
    Start,
    Csi,
    Ss3,
}

/// Terminal special characters from the saved termios snapshot.
pub(crate) struct Keys {
    pub erase: u8,
    pub kill: u8,
    pub werase: u8,
    pub eof: u8,
}

/// `None` marks a rejected byte or control character. It stays visible ("?")
/// and erasable, but any remaining one rejects the whole answer.
struct Item {
    ch: Option<char>,
    width: usize,
}

pub(crate) struct Editor {
    items: Vec<Item>,
    pending: Vec<u8>,
    need: usize,
    escape: Escape,
    overflow: bool,
    max: usize,
    keys: Keys,
}

fn key(code: u8, byte: u8) -> bool {
    // _POSIX_VDISABLE is 0 on Linux and 0xff on macOS.
    code != 0 && code != 0xff && code == byte
}

impl Editor {
    pub(crate) fn new(max: usize, keys: Keys) -> Self {
        Self {
            items: Vec::new(),
            pending: Vec::new(),
            need: 0,
            escape: Escape::None,
            overflow: false,
            max,
            keys,
        }
    }
    /// Characters kept for editing. Surrounding whitespace is trimmed before
    /// validation, so allow some slack, but never grow without bound.
    fn capacity(&self) -> usize {
        self.max.saturating_mul(2).saturating_add(64)
    }
    fn push(&mut self, ch: Option<char>, echo: &mut Vec<u8>) {
        if self.items.len() >= self.capacity() {
            self.overflow = true;
            return;
        }
        let width = match ch {
            Some(c) => {
                let mut buffer = [0; 4];
                echo.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
                width(c)
            }
            None => {
                echo.push(b'?');
                1
            }
        };
        self.items.push(Item { ch, width });
    }
    fn erase(&mut self, echo: &mut Vec<u8>) {
        // A partial UTF-8 sequence is part of the character being typed.
        if !self.pending.is_empty() {
            self.pending.clear();
            self.need = 0;
            return;
        }
        if let Some(item) = self.items.pop() {
            for _ in 0..item.width {
                echo.extend_from_slice(b"\x08 \x08");
            }
        }
    }
    fn finish(&mut self) -> Option<String> {
        if self.overflow || !self.pending.is_empty() {
            return None;
        }
        let text: Option<String> = self.items.iter().map(|item| item.ch).collect();
        crate::text_input(&text?, self.max)
    }
}

impl Editor {
    /// Feed one byte. Echo is appended for display only; it never changes the
    /// decision. Submission does not look past the terminating newline.
    pub(crate) fn feed(&mut self, byte: u8, echo: &mut Vec<u8>) -> Step {
        match self.escape {
            Escape::Start => {
                self.escape = match byte {
                    b'[' => Escape::Csi,
                    b'O' => Escape::Ss3,
                    _ => Escape::None,
                };
                return Step::More;
            }
            // Arrows/function keys are not editing commands; drop them whole
            // so stray "[A" bytes never join the answer.
            Escape::Csi => {
                if (0x40..=0x7e).contains(&byte) {
                    self.escape = Escape::None;
                }
                return Step::More;
            }
            Escape::Ss3 => {
                self.escape = Escape::None;
                return Step::More;
            }
            Escape::None => {}
        }
        if self.need > 0 && !(0x80..0xc0).contains(&byte) {
            // Truncated UTF-8 sequence: reject it, then handle this byte.
            self.pending.clear();
            self.need = 0;
            self.push(None, echo);
        }
        match byte {
            b'\r' | b'\n' => {
                echo.push(b'\n');
                return Step::Submit(self.finish());
            }
            b if key(self.keys.eof, b) && self.items.is_empty() && self.pending.is_empty() => {
                return Step::Eof;
            }
            b if key(self.keys.erase, b) || b == 0x7f || b == 0x08 => self.erase(echo),
            b if key(self.keys.kill, b) => {
                self.pending.clear();
                self.need = 0;
                while !self.items.is_empty() {
                    self.erase(echo);
                }
            }
            b if key(self.keys.werase, b) => {
                self.erase_word(echo);
            }
            0x1b => self.escape = Escape::Start,
            0x00..=0x1f => {
                // Other controls are rejected, not interpreted or echoed raw.
                // Terminal signal keys never get here: ISIG stays enabled.
                self.push(None, echo);
            }
            0x20..=0x7e => self.push(Some(byte as char), echo),
            _ => self.utf8(byte, echo),
        }
        Step::More
    }
}

impl Editor {
    fn utf8(&mut self, byte: u8, echo: &mut Vec<u8>) {
        if self.pending.is_empty() {
            self.need = match byte {
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => {
                    self.push(None, echo);
                    return;
                }
            };
        }
        self.pending.push(byte);
        if self.pending.len() < self.need {
            return;
        }
        match std::str::from_utf8(&self.pending) {
            Ok(text) => {
                let ch = text.chars().next();
                self.push(ch, echo);
            }
            Err(_) => self.push(None, echo),
        }
        self.pending.clear();
        self.need = 0;
    }
    fn erase_word(&mut self, echo: &mut Vec<u8>) {
        while self
            .items
            .last()
            .is_some_and(|i| i.ch == Some(' ') || i.ch == Some('\t'))
        {
            self.erase(echo);
        }
        while self
            .items
            .last()
            .is_some_and(|i| i.ch != Some(' ') && i.ch != Some('\t'))
        {
            self.erase(echo);
        }
    }
}

/// Display width for the small set of characters these prompts echo: control
/// and combining marks take no column, East-Asian wide/fullwidth take two,
/// everything else one. This drives erase echo only, not the stored answer.
fn width(c: char) -> usize {
    let n = c as u32;
    if c.is_control() {
        return 0;
    }
    if matches!(n,
        0x0300..=0x036f | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff |
        0x20d0..=0x20ff | 0xfe20..=0xfe2f | 0x200b..=0x200f | 0xfeff)
    {
        return 0;
    }
    if matches!(n,
        0x1100..=0x115f | 0x2329 | 0x232a | 0x2e80..=0x303e | 0x3041..=0x33ff |
        0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xa000..=0xa4cf | 0xa960..=0xa97f |
        0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xfe10..=0xfe19 | 0xfe30..=0xfe6f |
        0xff00..=0xff60 | 0xffe0..=0xffe6 | 0x1f300..=0x1faff | 0x20000..=0x3fffd)
    {
        return 2;
    }
    1
}

impl Editor {
    /// Echo the current answer again after a resume.
    pub(crate) fn redraw(&self, echo: &mut Vec<u8>) {
        for item in &self.items {
            match item.ch {
                Some(c) => {
                    let mut buffer = [0; 4];
                    echo.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
                }
                None => echo.push(b'?'),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Editor, Keys, Step};

    fn editor(max: usize) -> Editor {
        Editor::new(
            max,
            Keys {
                erase: 0x7f,
                kill: 0x15,
                werase: 0x17,
                eof: 0x04,
            },
        )
    }
    fn run(max: usize, input: &[u8]) -> (Step, Vec<u8>, usize) {
        let mut e = editor(max);
        let mut echo = Vec::new();
        for (index, byte) in input.iter().enumerate() {
            let step = e.feed(*byte, &mut echo);
            if step != Step::More {
                return (step, echo, index + 1);
            }
        }
        (Step::More, echo, input.len())
    }
    fn answer(max: usize, input: &[u8]) -> Option<String> {
        match run(max, input).0 {
            Step::Submit(answer) => answer,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn long_cjk_lines_are_exact_and_bounded_by_characters() {
        for (count, valid) in [(1999, true), (2000, true), (2001, false)] {
            let text = "中".repeat(count);
            let got = answer(2000, format!("{text}\n").as_bytes());
            assert_eq!(got.as_deref(), valid.then_some(text.as_str()), "{count}");
        }
        let mixed = "中x".repeat(1000);
        assert_eq!(answer(2000, format!("{mixed}\r").as_bytes()), Some(mixed));
        // Oversized pastes stay bounded: storage stops, the line still ends.
        assert_eq!(run(10, &[b'x'; 100_000]).0, Step::More);
        assert_eq!(answer(10, &[&[b'x'; 100_000][..], b"\n"].concat()), None);
    }

    #[test]
    fn submission_never_consumes_the_next_line() {
        let (step, _, used) = run(2000, b"answer\nchild-data\n");
        assert_eq!(step, Step::Submit(Some("answer".into())));
        assert_eq!(used, 7);
    }

    #[test]
    fn editing_is_character_based_not_byte_based() {
        assert_eq!(answer(10, "中文\x7f字\n".as_bytes()), Some("中字".into()));
        assert_eq!(answer(10, b"abc\x08d\n"), Some("abd".into()));
        assert_eq!(answer(10, "垃圾\x15好\n".as_bytes()), Some("好".into()));
        assert_eq!(
            answer(20, b"keep this  word\x17\n"),
            Some("keep this".into())
        );
        // Erase removes the visual width of a wide character.
        let (_, echo, _) = run(10, "中\x7f".as_bytes());
        assert!(echo.ends_with(b"\x08 \x08\x08 \x08"), "{echo:?}");
        // Erasing in the middle of a multibyte sequence drops the partial bytes.
        assert_eq!(answer(10, b"a\xe4\xb8\x7fb\n"), Some("ab".into()));
    }

    #[test]
    fn escapes_controls_and_invalid_utf8_never_join_the_answer() {
        assert_eq!(
            answer(20, b"go\x1b[A\x1b[1;5Cup\x1bOP!\n"),
            Some("goup!".into())
        );
        assert_eq!(answer(20, b"tab\there\n"), None);
        assert_eq!(answer(20, b"bad\xff\n"), None);
        assert_eq!(answer(20, b"cut\xe4\xb8x\n"), None);
        assert_eq!(answer(20, b"\xe4\xb8\n"), None);
        // A rejected byte can be erased, after which the answer is valid.
        assert_eq!(answer(20, b"ok\x01\x7f\n"), Some("ok".into()));
    }

    #[test]
    fn blank_eof_and_disabled_keys() {
        assert_eq!(answer(20, b"   \n"), None);
        assert_eq!(run(20, b"\x04").0, Step::Eof);
        // EOF on a nonempty line is not end-of-file; it is a rejected control.
        assert_eq!(answer(20, b"x\x04\n"), None);
        let mut e = Editor::new(
            20,
            Keys {
                erase: 0xff,
                kill: 0,
                werase: 0,
                eof: 0,
            },
        );
        let mut echo = Vec::new();
        assert_eq!(e.feed(0, &mut echo), Step::More);
        assert_eq!(e.feed(b'\n', &mut echo), Step::Submit(None));
    }
}
