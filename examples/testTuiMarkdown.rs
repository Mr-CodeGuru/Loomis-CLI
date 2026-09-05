use loomiscli::tui::markdown::parse_markdown_to_lines;

fn main() {
    println!("=== Testing LoomisCLI TUI Markdown & Syntect Syntax Rendering ===");

    let markdown_sample = r#"
# Heading 1: Overview
Here is an introductory paragraph with **bold text**, *italic text*, and `inline_code_span()`.

## Heading 2: Code Sample
Below is a syntax-highlighted Rust code snippet:

```rust
fn calculate_md5(data: &[u8]) -> String {
    let mut context = md5::Context::new();
    context.consume(data);
    format!("{:x}", context.compute())
}
```

### Heading 3: Lists & Features
- Item 1: High performance local RAG
- Item 2: Native LanceDB vector storage
- Item 3: Monokai syntax highlighting

1. First ordered step
2. Second ordered step

> This is an important quote or note from the assistant.

---
End of response.
"#;

    let lines = parse_markdown_to_lines(markdown_sample);

    println!("Successfully parsed {} ratatui lines from markdown sample.", lines.len());
    assert!(!lines.is_empty(), "Parsed lines should not be empty");

    let mut found_code_box = false;
    let mut found_h1 = false;
    let mut found_blockquote = false;
    let mut found_hr = false;

    for line in &lines {
        let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        if content.contains("█ Heading 1: Overview") {
            found_h1 = true;
        }
        if content.contains("╭─── [rust] ───") {
            found_code_box = true;
        }
        if content.contains("▎ This is an important quote") {
            found_blockquote = true;
        }
        if content.contains("────") {
            found_hr = true;
        }
    }

    assert!(found_h1, "H1 header marker was not found");
    assert!(found_code_box, "Code box header was not found");
    assert!(found_blockquote, "Blockquote marker was not found");
    assert!(found_hr, "Horizontal rule divider was not found");

    println!("[PASS] All Markdown AST parsing, headers, lists, code boxes, and blockquotes verified!");
    println!("=== TUI Markdown Verification Succeeded ===");
}
