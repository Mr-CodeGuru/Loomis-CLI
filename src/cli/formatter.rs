use std::io::{self, Write};

pub const RESET: &str = "\x1b[0m";
pub const BORDER_GRAY: &str = "\x1b[90m";
pub const LANG_CYAN: &str = "\x1b[1;36m";
pub const KW_MAGENTA: &str = "\x1b[1;35m";
pub const KW_CYAN: &str = "\x1b[1;36m";
pub const STR_GREEN: &str = "\x1b[32m";
pub const NUM_YELLOW: &str = "\x1b[33m";
pub const COMMENT_GRAY: &str = "\x1b[90m";
pub const FN_BLUE: &str = "\x1b[1;34m";

/// Real-time streaming formatter that parses code fences on-the-fly,
/// encloses code blocks in a bordered box, and applies Dark Modern / Monokai syntax highlighting.
pub struct StreamingCodeFormatter {
    in_code_block: bool,
    current_lang: String,
    line_buffer: String,
}

impl StreamingCodeFormatter {
    pub fn new() -> Self {
        Self {
            in_code_block: false,
            current_lang: String::new(),
            line_buffer: String::new(),
        }
    }

    /// Process an incoming token chunk from the LLM stream.
    pub fn process_chunk(&mut self, chunk: &str) -> io::Result<()> {
        self.line_buffer.push_str(chunk);

        while let Some(pos) = self.line_buffer.find('\n') {
            let line: String = self.line_buffer.drain(..=pos).collect();
            let trimmed_line = line.trim_end_matches(&['\r', '\n'][..]);
            self.render_line(trimmed_line)?;
        }

        Ok(())
    }

    /// Flush any remaining partial line when stream finishes.
    pub fn finish(&mut self) -> io::Result<()> {
        if !self.line_buffer.is_empty() {
            let rem = std::mem::take(&mut self.line_buffer);
            self.render_line(&rem)?;
        }

        if self.in_code_block {
            self.in_code_block = false;
            println!("{BORDER_GRAY}╰───────────────────────────────────────────────────────────{RESET}");
            io::stdout().flush()?;
        }

        Ok(())
    }

    fn render_line(&mut self, line: &str) -> io::Result<()> {
        let trimmed = line.trim();

        if !self.in_code_block {
            if trimmed.starts_with("```") {
                // Entering code fence
                self.in_code_block = true;
                let lang = trimmed.trim_start_matches('`').trim();
                self.current_lang = if lang.is_empty() {
                    "code".to_string()
                } else {
                    lang.to_string()
                };

                println!(
                    "{BORDER_GRAY}╭─── {LANG_CYAN}[{}]{BORDER_GRAY} ──────────────────────────────────────────{RESET}",
                    self.current_lang
                );
                io::stdout().flush()?;
            } else {
                // Normal prose
                println!("{line}");
                io::stdout().flush()?;
            }
        } else {
            if trimmed.starts_with("```") {
                // Exiting code fence
                self.in_code_block = false;
                self.current_lang.clear();
                println!("{BORDER_GRAY}╰───────────────────────────────────────────────────────────{RESET}");
                io::stdout().flush()?;
            } else {
                // Code line inside block: frame with border and apply syntax theme
                let highlighted = highlight_syntax(line, &self.current_lang);
                println!("{BORDER_GRAY}│ {RESET}{highlighted}");
                io::stdout().flush()?;
            }
        }

        Ok(())
    }
}

/// Apply Dark Modern / Monokai syntax highlighting to a single code line.
pub fn highlight_syntax(line: &str, _lang: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut out = String::with_capacity(line.len() * 2);
    let mut i = 0;

    while i < len {
        // 1. Comments: check for # or //
        if chars[i] == '#' || (chars[i] == '/' && i + 1 < len && chars[i + 1] == '/') {
            out.push_str(COMMENT_GRAY);
            for &c in &chars[i..] {
                out.push(c);
            }
            out.push_str(RESET);
            break;
        }

        // 2. String literals: single or double quotes
        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            out.push_str(STR_GREEN);
            out.push(quote);
            i += 1;
            while i < len {
                let c = chars[i];
                out.push(c);
                if c == '\\' && i + 1 < len {
                    i += 1;
                    out.push(chars[i]);
                } else if c == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push_str(RESET);
            continue;
        }

        // 3. Numbers
        if chars[i].is_ascii_digit() && (i == 0 || !chars[i - 1].is_alphanumeric() && chars[i - 1] != '_') {
            out.push_str(NUM_YELLOW);
            while i < len && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == 'x' || chars[i] == 'b' || chars[i] == 'f' || chars[i] == '_') {
                out.push(chars[i]);
                i += 1;
            }
            out.push_str(RESET);
            continue;
        }

        // 4. Identifiers / Keywords
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();

            // Check if function call (immediately followed by '(' ignoring spaces)
            let mut is_fn_call = false;
            let mut peek = i;
            while peek < len && chars[peek] == ' ' {
                peek += 1;
            }
            if peek < len && chars[peek] == '(' {
                is_fn_call = true;
            }

            match word.as_str() {
                // Magenta keywords (declarations, imports, definitions)
                "def" | "class" | "fn" | "struct" | "enum" | "impl" | "pub" | "type"
                | "let" | "mut" | "const" | "import" | "from" | "as" => {
                    out.push_str(KW_MAGENTA);
                    out.push_str(&word);
                    out.push_str(RESET);
                }
                // Cyan keywords (control flow, operators, async)
                "return" | "if" | "else" | "elif" | "for" | "while" | "loop" | "in"
                | "is" | "not" | "and" | "or" | "match" | "try" | "except" | "finally"
                | "with" | "async" | "await" | "yield" | "break" | "continue" | "use"
                | "mod" | "where" => {
                    out.push_str(KW_CYAN);
                    out.push_str(&word);
                    out.push_str(RESET);
                }
                // Constants / Special values
                "True" | "False" | "None" | "true" | "false" | "Some" | "Ok" | "Err"
                | "self" | "Self" => {
                    out.push_str(NUM_YELLOW);
                    out.push_str(&word);
                    out.push_str(RESET);
                }
                _ => {
                    if is_fn_call {
                        out.push_str(FN_BLUE);
                        out.push_str(&word);
                        out.push_str(RESET);
                    } else {
                        out.push_str(&word);
                    }
                }
            }
            continue;
        }

        // Punctuation and whitespace
        out.push(chars[i]);
        i += 1;
    }

    out
}
