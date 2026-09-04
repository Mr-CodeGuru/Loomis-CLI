use crate::db::SearchResult;
use super::client::ChatMessage;

pub fn build_rag_messages(
    query: &str,
    context_chunks: &[SearchResult],
    history: &[ChatMessage],
) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    let system_instruction = "\
You are Loomis, an expert terminal-based local code assistant.
Help the user with their programming inquiry using the retrieved repository snippets below.
Explain how the code works and cite the relevant file paths and functions.
If the snippets provide examples or related implementations, explain how the user can use or adapt them to solve their problem.
Be direct, helpful, and concise.";

    messages.push(ChatMessage {
        role: "system".to_string(),
        content: system_instruction.to_string(),
    });

    // Append recent sanitized session turns (keep at most the last 4 messages = 2 conversation turns)
    let history_slice = if history.len() > 4 {
        &history[history.len() - 4..]
    } else {
        history
    };

    for msg in history_slice {
        // Skip poisoned refusals from contaminating context
        if msg.role == "assistant" && msg.content.trim().starts_with("I can't answer that") {
            continue;
        }
        messages.push(msg.clone());
    }

    // Format retrieved context chunks with a reasonable length cap per snippet (max 1500 chars)
    // to preserve llama-server's 4096 token context window
    let mut context_block = String::new();
    for (i, chunk) in context_chunks.iter().enumerate() {
        let symbol_label = if chunk.extracted_name.is_empty() {
            "unnamed block"
        } else {
            &chunk.extracted_name
        };

        let snippet_text = if chunk.text.len() > 1500 {
            format!("{}...\n[truncated]", &chunk.text[..1500])
        } else {
            chunk.text.trim().to_string()
        };

        context_block.push_str(&format!(
            "\n[Snippet {}] File: {} | Symbol: {} | Lang: {}\n```{}\\n{}\\n```\n",
            i + 1,
            chunk.path,
            symbol_label,
            chunk.language,
            chunk.language,
            snippet_text
        ));
    }

    // User question placed FIRST so the 1B model immediately grasps the intent,
    // followed by supporting context snippets, followed by generation instructions.
    let user_content = format!(
        "User Question:\n{}\n\nContext Snippets from Repository:\n{}\nPlease answer the user's question directly using the snippets above, citing the file path and function.",
        query, context_block
    );

    messages.push(ChatMessage {
        role: "user".to_string(),
        content: user_content,
    });

    messages
}
