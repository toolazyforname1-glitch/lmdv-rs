//! A small, theme-native syntax highlighter.
//!
//! lmdv draws fenced code with its own syntax inks, so the TUI does the same:
//! a compact lexer that colours comments, strings, numbers, keywords, calls,
//! types and attributes. It is deliberately language-approximate — it never
//! fails, and it stays coherent with every palette.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::theme::Palette;

const KEYWORDS: &[&str] = &[
    // Rust
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "yield",
    // Python
    "and", "assert", "class", "def", "del", "elif", "except", "finally", "from", "global",
    "import", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "True", "False",
    "try", "with",
    // JS / TS
    "catch", "constructor", "delete", "export", "extends", "function", "implements", "instanceof",
    "interface", "new", "null", "private", "protected", "public", "readonly", "switch", "this",
    "throw", "typeof", "undefined", "var", "void",
    // Go / C / shell
    "chan", "defer", "fallthrough", "func", "go", "goto", "package", "range", "select", "case",
    "default", "map", "auto", "char", "double", "float", "int", "long", "short", "signed",
    "sizeof", "unsigned", "struct", "typedef", "union", "volatile", "echo", "fi", "then", "do",
    "done", "esac", "export", "local", "readonly", "sudo",
];

fn hash_comments(lang: &str) -> bool {
    matches!(
        lang,
        "py" | "python" | "sh" | "bash" | "shell" | "zsh" | "toml" | "yaml" | "yml" | "make"
            | "dockerfile" | "ruby" | "rb" | "r" | "perl" | "pl" | "nim" | "elixir" | "ex"
            | "conf" | "ini" | "gitignore"
    )
}

fn slash_comments(lang: &str) -> bool {
    !hash_comments(lang) || matches!(lang, "javascript" | "js" | "ts" | "rust")
}

/// Colour one line of code. Returns spans with foreground ink only; the caller
/// paints the code block background across the whole row.
pub fn code_line(lang: &str, text: &str, p: &Palette) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut i = 0usize;
    let hash = hash_comments(lang);
    let slash = slash_comments(lang);

    let push = |out: &mut Vec<Span<'static>>, text: String, color: ratatui::style::Color| {
        if !text.is_empty() {
            out.push(Span::styled(text, Style::new().fg(color)));
        }
    };

    while i < chars.len() {
        let c = chars[i];
        // Comments.
        if slash && c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            let rest: String = chars[i..].iter().collect();
            push(&mut out, rest, p.syn_com);
            break;
        }
        if slash && c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            let rest: String = chars[i..].iter().collect();
            push(&mut out, rest, p.syn_com);
            break;
        }
        if hash && c == '#' {
            if lang.starts_with("rust") && chars.get(i + 1) == Some(&'[') {
                let mut j = i + 1;
                while j < chars.len() && chars[j] != ']' {
                    j += 1;
                }
                let attr: String = chars[i..(j + 1).min(chars.len())].iter().collect();
                push(&mut out, attr, p.syn_attr);
                i = (j + 1).min(chars.len());
                continue;
            }
            let rest: String = chars[i..].iter().collect();
            push(&mut out, rest, p.syn_com);
            break;
        }
        if c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' && matches!(lang, "sql" | "lua" | "hs" | "haskell") {
            let rest: String = chars[i..].iter().collect();
            push(&mut out, rest, p.syn_com);
            break;
        }
        // Strings.
        if c == '"' || c == '\'' || c == '`' {
            let quote = c;
            let mut j = i + 1;
            let mut escaped = false;
            while j < chars.len() {
                let ch = chars[j];
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    j += 1;
                    break;
                }
                j += 1;
            }
            let raw: String = chars[i..j].iter().collect();
            push(&mut out, raw, p.syn_str);
            i = j;
            continue;
        }
        // Numbers.
        if c.is_ascii_digit() {
            let mut j = i;
            while j < chars.len()
                && (chars[j].is_ascii_alphanumeric() || chars[j] == '.' || chars[j] == '_')
            {
                j += 1;
            }
            let raw: String = chars[i..j].iter().collect();
            push(&mut out, raw, p.syn_num);
            i = j;
            continue;
        }
        // Attributes and decorators.
        if c == '@' {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '.') {
                j += 1;
            }
            let raw: String = chars[i..j].iter().collect();
            push(&mut out, raw, p.syn_attr);
            i = j;
            continue;
        }
        // Identifiers.
        if c.is_alphabetic() || c == '_' {
            let mut j = i;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            let word: String = chars[i..j].iter().collect();
            let mut k = j;
            while k < chars.len() && chars[k].is_whitespace() {
                k += 1;
            }
            let color = if KEYWORDS.contains(&word.as_str()) {
                p.syn_kw
            } else if chars.get(k) == Some(&'(') {
                p.syn_fun
            } else if word
                .chars()
                .next()
                .is_some_and(|first| first.is_uppercase())
                && word.chars().any(|ch| ch.is_lowercase())
            {
                p.syn_typ
            } else {
                p.text
            };
            if KEYWORDS.contains(&word.as_str()) {
                out.push(Span::styled(word, Style::new().fg(color).add_modifier(Modifier::BOLD)));
            } else {
                push(&mut out, word, color);
            }
            i = j;
            continue;
        }
        // Punctuation and the rest.
        if "(){}[]<>+-=*/%&|!?:;,.".contains(c) {
            push(&mut out, c.to_string(), p.syn_punct);
        } else {
            push(&mut out, c.to_string(), p.text);
        }
        i += 1;
    }
    out
}
