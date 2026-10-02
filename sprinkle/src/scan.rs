//! Deliberately small EOF recognizer, not a parser. Unknown constructs are skipped.
pub fn safe(text: &str, ext: &str) -> bool {
    if text.is_empty() || !text.ends_with('\n') || text.contains('\0') {
        return false;
    }
    let crlf = text.contains("\r\n");
    if text.replace("\r\n", "").contains('\r') || (crlf && text.replace("\r\n", "").contains('\n'))
    {
        return false;
    }
    let header = text.lines().take(20).collect::<Vec<_>>().join("\n");
    if header.contains("@generated")
        || (header.contains("Code generated") && header.contains("DO NOT EDIT"))
    {
        return false;
    }
    if ext == "md" {
        return markdown(text);
    }
    let rust = ext == "rs";
    let go = ext == "go";
    let c = !rust && !go;
    // Translation phase line splicing and conditional preprocessing are intentionally excluded.
    if c && (text.contains("\\\n")
        || text.contains("\\\r\n")
        || text.lines().any(|l| {
            l.trim_start().starts_with('#')
                && !l.trim_start().starts_with("#include ")
                && l.trim() != "#pragma once"
        }))
    {
        return false;
    }
    if c && (text.contains("??")
        || text.contains("%:")
        || text.contains("<%")
        || text.contains("%>"))
    {
        return false;
    }
    let b = text.as_bytes();
    let mut i = 0;
    let mut braces = Vec::new();
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            i += 2;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            i += 2;
            let mut depth = 1;
            while i < b.len() && depth > 0 {
                if rust && b[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if depth != 0 {
                return false;
            }
            continue;
        }
        // Rust raw strings (also reached after b/c prefixes).
        if rust && b[i] == b'r' {
            let mut j = i + 1;
            while j < b.len() && b[j] == b'#' {
                j += 1;
            }
            if j < b.len() && b[j] == b'"' {
                let hashes = j - i - 1;
                i = j + 1;
                let mut closed = false;
                while i < b.len() {
                    if b[i] == b'"'
                        && b.get(i + 1..i + 1 + hashes)
                            .is_some_and(|s| s.iter().all(|c| *c == b'#'))
                    {
                        i += 1 + hashes;
                        closed = true;
                        break;
                    }
                    i += 1;
                }
                if !closed {
                    return false;
                }
                continue;
            }
        }
        if c && b[i..].starts_with(b"R\"") {
            let start = i + 2;
            let Some(n) = b[start..].iter().position(|c| *c == b'(') else {
                return false;
            };
            if n > 16
                || b[start..start + n]
                    .iter()
                    .any(|c| c.is_ascii_whitespace() || matches!(c, b'\\' | b')'))
            {
                return false;
            }
            let mut end = vec![b')'];
            end.extend_from_slice(&b[start..start + n]);
            end.push(b'"');
            let from = start + n + 1;
            let Some(n) = b[from..].windows(end.len()).position(|s| s == end) else {
                return false;
            };
            i = from + n + end.len();
            continue;
        }
        if go && b[i] == b'`' {
            i += 1;
            while i < b.len() && b[i] != b'`' {
                i += 1;
            }
            if i == b.len() {
                return false;
            }
            i += 1;
            continue;
        }
        if b[i] == b'\'' && rust {
            // ASCII lifetime/label, as opposed to a character literal. Unknown Unicode lifetimes skip.
            let mut j = i + 1;
            if j < b.len() && (b[j].is_ascii_alphabetic() || b[j] == b'_') {
                j += 1;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                    j += 1;
                }
                if b.get(j) != Some(&b'\'') {
                    i = j;
                    continue;
                }
            }
        }
        if matches!(b[i], b'"' | b'\'') {
            let quote = b[i];
            i += 1;
            let mut closed = false;
            while i < b.len() {
                if b[i] == quote {
                    i += 1;
                    closed = true;
                    break;
                }
                if b[i] == b'\\' {
                    i += 2;
                } else {
                    if !rust && b[i] == b'\n' {
                        return false;
                    }
                    i += 1;
                }
            }
            if !closed {
                return false;
            }
            continue;
        }
        match b[i] {
            b'{' | b'(' | b'[' => braces.push(b[i]),
            b'}' | b')' | b']' => {
                let expected = match b[i] {
                    b'}' => b'{',
                    b')' => b'(',
                    _ => b'[',
                };
                if braces.pop() != Some(expected) {
                    return false;
                }
            }
            b'`' | b'\\' => return false,
            _ => (),
        }
        i += 1;
    }
    braces.is_empty()
}
fn markdown(text: &str) -> bool {
    if text.starts_with("---") || text.starts_with("+++") {
        return false;
    }
    let mut fence: Option<(u8, usize)> = None;
    let mut comment = false;
    for line in text.lines() {
        let t = line.trim_start();
        if !comment && (t.starts_with("```") || t.starts_with("~~~")) {
            let c = t.as_bytes()[0];
            let n = t.bytes().take_while(|b| *b == c).count();
            match fence {
                None => fence = Some((c, n)),
                Some((old, len)) if c == old && n >= len && t[n..].trim().is_empty() => {
                    fence = None
                }
                _ => (),
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let mut rest = line;
        loop {
            if comment {
                match rest.find("-->") {
                    Some(n) => {
                        rest = &rest[n + 3..];
                        comment = false;
                    }
                    None => break,
                }
            } else {
                match rest.find('<') {
                    Some(n) if rest[n..].starts_with("<!--") => {
                        comment = true;
                        rest = &rest[n + 4..];
                    }
                    Some(_) => return false,
                    None => break,
                }
            }
        }
    }
    fence.is_none() && !comment
}
#[cfg(test)]
mod tests {
    use super::safe;
    #[test]
    fn boundaries() {
        for (s, e) in [
            ("fn main() {}\n", "rs"),
            ("/* outer /* nested */ */\n", "rs"),
            ("let a = r###\"/*\"###;\n", "rs"),
            ("fn f<'a>(a: &'a str) {}\n", "rs"),
            ("package p\nvar s = `hi`\n", "go"),
            ("int main() { return 0; }\n", "c"),
            ("auto s=R\"x(/*)x\";\n", "cpp"),
            ("# hi\n\ntext\n", "md"),
            ("```rust\nx\n```\n", "md"),
        ] {
            assert!(safe(s, e), "{e}: {s}");
        }
        for (s, e) in [
            ("fn main() {}", "rs"),
            ("/* /* */\n", "rs"),
            ("r###\"abc\"##\n", "rs"),
            ("#if X\nint x;\n", "c"),
            ("// hello\\\n", "cpp"),
            ("var s = `hi\n", "go"),
            ("```\nhi\n", "md"),
            ("<!--\n", "md"),
            ("<div>\n", "md"),
            ("---\ntitle: hi\n---\n", "md"),
            ("hello\r\nworld\n", "md"),
            ("// @generated\nfn f(){}\n", "rs"),
        ] {
            assert!(!safe(s, e), "{e}: {s}");
        }
    }
}
