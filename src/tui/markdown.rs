use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

pub fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

pub fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

pub fn get_active_theme() -> &'static Theme {
    let themes = get_theme_set();
    themes
        .themes
        .get("base16-ocean.dark")
        .or_else(|| themes.themes.get("Solarized (dark)"))
        .unwrap_or_else(|| themes.themes.values().next().unwrap())
}

/// Highlight a block of code using `syntect` and wrap in rounded box borders.
pub fn highlight_code_block<'a>(code: &str, lang: &str) -> Vec<Line<'a>> {
    let ss = get_syntax_set();
    let theme = get_active_theme();
    let syntax = ss
        .find_syntax_by_token(lang)
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();

    let display_lang = if lang.trim().is_empty() {
        "code"
    } else {
        lang.trim()
    };

    // Header border with language badge
    lines.push(Line::from(vec![
        Span::styled("╭─── [", Style::default().fg(Color::DarkGray)),
        Span::styled(
            display_lang.to_string(),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "] ──────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    for line in code.lines() {
        let mut spans = vec![Span::styled(
            "│ ",
            Style::default().fg(Color::DarkGray),
        )];

        let line_with_nl = format!("{line}\n");
        if let Ok(ranges) = highlighter.highlight_line(&line_with_nl, ss) {
            for (style, text) in ranges {
                let text_trimmed = text.trim_end_matches('\n');
                if !text_trimmed.is_empty() {
                    let fg = Color::Rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    );
                    spans.push(Span::styled(
                        text_trimmed.to_string(),
                        Style::default().fg(fg),
                    ));
                }
            }
        } else {
            spans.push(Span::raw(line.to_string()));
        }

        lines.push(Line::from(spans));
    }

    // Footer border
    lines.push(Line::from(vec![Span::styled(
        "╰───────────────────────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    )]));

    lines
}

/// Parse a markdown string into fully styled ratatui `Line` elements.
/// Handles headers, bold, italic, inline code, lists, blockquotes, horizontal rules,
/// and syntax-highlighted code blocks.
pub fn parse_markdown_to_lines<'a>(markdown: &str) -> Vec<Line<'a>> {
    let parser = Parser::new(markdown);
    let mut lines: Vec<Line<'a>> = Vec::new();
    let mut current_spans: Vec<Span<'a>> = Vec::new();

    let mut is_bold = false;
    let mut is_italic = false;
    let mut in_blockquote = false;
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_content = String::new();
    let mut current_heading: Option<HeadingLevel> = None;
    let mut list_index: Option<u64> = None;
    let mut list_depth: usize = 0;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    current_heading = Some(level);
                    let prefix = match level {
                        HeadingLevel::H1 => "█ ",
                        HeadingLevel::H2 => "▓ ",
                        _ => "▌ ",
                    };
                    current_spans.push(Span::styled(
                        prefix.to_string(),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ));
                }
                Tag::Paragraph => {
                    // Start of paragraph
                }
                Tag::BlockQuote(_) => {
                    in_blockquote = true;
                }
                Tag::CodeBlock(kind) => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    in_code_block = true;
                    code_lang = match kind {
                        CodeBlockKind::Fenced(l) => l.to_string(),
                        CodeBlockKind::Indented => "code".to_string(),
                    };
                    code_content.clear();
                }
                Tag::List(first_num) => {
                    list_depth += 1;
                    list_index = first_num;
                }
                Tag::Item => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    let indent = "  ".repeat(list_depth.saturating_sub(1));
                    let bullet = if let Some(idx) = list_index {
                        format!("{indent}{idx}. ")
                    } else {
                        format!("{indent}• ")
                    };
                    current_spans.push(Span::styled(
                        bullet,
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ));
                    if let Some(idx) = &mut list_index {
                        *idx += 1;
                    }
                }
                Tag::Emphasis => {
                    is_italic = true;
                }
                Tag::Strong => {
                    is_bold = true;
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    current_heading = None;
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
                TagEnd::Paragraph => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    lines.push(Line::from(String::new())); // empty line separator
                }
                TagEnd::BlockQuote(_) => {
                    in_blockquote = false;
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    let block_lines = highlight_code_block(&code_content, &code_lang);
                    lines.extend(block_lines);
                    lines.push(Line::from(String::new()));
                }
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                    if list_depth == 0 {
                        list_index = None;
                    }
                }
                TagEnd::Item => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                }
                TagEnd::Emphasis => {
                    is_italic = false;
                }
                TagEnd::Strong => {
                    is_bold = false;
                }
                _ => {}
            },
            Event::Text(text) => {
                if in_code_block {
                    code_content.push_str(&text);
                } else {
                    let mut style = Style::default();
                    if is_bold {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if is_italic {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    if in_blockquote {
                        style = style.fg(Color::LightCyan).add_modifier(Modifier::ITALIC);
                    }
                    if let Some(level) = current_heading {
                        let color = match level {
                            HeadingLevel::H1 => Color::Yellow,
                            HeadingLevel::H2 => Color::LightYellow,
                            _ => Color::White,
                        };
                        style = style.fg(color).add_modifier(Modifier::BOLD);
                    }

                    if in_blockquote && current_spans.is_empty() {
                        current_spans.push(Span::styled(
                            "▎ ",
                            Style::default().fg(Color::Cyan),
                        ));
                    }

                    current_spans.push(Span::styled(text.to_string(), style));
                }
            }
            Event::Code(inline_code) => {
                let inline_style = Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Rgb(40, 44, 52))
                    .add_modifier(Modifier::BOLD);
                current_spans.push(Span::styled(
                    format!(" {inline_code} "),
                    inline_style,
                ));
            }
            Event::Rule => {
                if !current_spans.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
                lines.push(Line::from(Span::styled(
                    "────────────────────────────────────────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            Event::SoftBreak | Event::HardBreak => {
                if !current_spans.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
            }
            _ => {}
        }
    }

    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }

    lines
}
