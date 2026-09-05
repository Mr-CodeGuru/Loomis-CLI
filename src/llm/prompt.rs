use crate::db::SearchResult;
use super::client::ChatMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeIntent {
    Code,
    Chat,
}

/// Fallback classifier used only when LLM server is unreachable.
/// Defaults conservatively to Chat to avoid generating unprompted code.
pub fn fallback_classify_code_intent(_query: &str) -> CodeIntent {
    CodeIntent::Chat
}

pub fn build_rag_messages(
    query: &str,
    context_chunks: &[SearchResult],
    history: &[ChatMessage],
    intent: CodeIntent,
) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    let system_instruction = match intent {
        CodeIntent::Chat => "\
You are Loomis, an expert terminal-based local code assistant.
You maintain awareness of this conversation session. The chat history contains previous messages exchanged with the user in this session. Answer questions about previous turns accurately.
Respond helpfully and concisely. Do not output unrequested code blocks.",

        CodeIntent::Code => "\
You are Loomis, an expert terminal-based local code assistant.
You maintain awareness of this conversation session and previous discussion.
You help the user by writing clean code that follows the style, patterns, and conventions found in the retrieved repository snippets.

STRICT INSTRUCTIONS:
1. Ground your implementation directly in the retrieved code snippets.
2. Provide a single, focused code solution answering the user's inquiry.
3. Only import libraries and call functions that are strictly necessary. Never add dead or unused imports.
4. In your explanation, describe ONLY the code you actually wrote. Do NOT list or discuss functions, libraries, or modules not present in your code.
5. Explicitly cite which retrieved snippet file path and symbol your code adapts.
6. If the user is asking to modify, extend, or refine code from a prior turn in this session, adapt the previous solution while respecting repository patterns.
7. Keep your response direct, clean, and concise.",
    };

    messages.push(ChatMessage {
        role: "system".to_string(),
        content: system_instruction.to_string(),
    });

    // Adaptive history window:
    // - For Chat: up to 10 messages (5 turns), max 4000 characters total.
    // - For Code: up to 6 messages (3 turns), max 1500 characters total, reserving tokens for retrieved snippets.
    let (max_msgs, max_chars) = match intent {
        CodeIntent::Chat => (10, 4000),
        CodeIntent::Code => (6, 1500),
    };

    let start_idx = history.len().saturating_sub(max_msgs);
    let mut history_slice = &history[start_idx..];

    // Calculate total character count and trim oldest turns if exceeding budget
    let mut total_chars: usize = history_slice.iter().map(|m| m.content.len()).sum();
    while total_chars > max_chars && history_slice.len() > 2 {
        total_chars -= history_slice[0].content.len() + history_slice[1].content.len();
        history_slice = &history_slice[2..];
    }

    for msg in history_slice {
        // Skip poisoned refusals from contaminating context
        if msg.role == "assistant" && msg.content.trim().starts_with("I can't answer that") {
            continue;
        }
        messages.push(msg.clone());
    }

    let user_content = match intent {
        CodeIntent::Chat => query.to_string(),
        CodeIntent::Code => {
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

            format!(
                "User Request:\n{}\n\nContext Snippets from Repository:\n{}\nInstructions:\nProvide a single, focused code solution adapting the codebase conventions above. Only import what is used. Then provide a brief explanation citing the specific snippet file and function you adapted.",
                query, context_block
            )
        }
    };

    messages.push(ChatMessage {
        role: "user".to_string(),
        content: user_content,
    });

    messages
}
