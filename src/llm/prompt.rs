use crate::db::SearchResult;
use super::client::ChatMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeIntent {
    Code,
    Chat,
}

pub fn fallback_classify_code_intent(query: &str) -> CodeIntent {
    let lower = query.trim().to_lowercase();

    // Strict allow-list keywords for code-seeking requests
    let code_patterns = [
        "write a", "write python", "write rust", "write code", "implement",
        "generate a", "create a function", "create a class", "create a script",
        "refactor", "extend the", "build a function", "how would you write",
        "how would you code", "how would you implement", "can you write",
        "can you implement", "can you code", "show me an example of code",
        "give me a function", "def ", "class ", "fn "
    ];

    for pat in &code_patterns {
        if lower.contains(pat) {
            return CodeIntent::Code;
        }
    }

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
Respond to the user politely, helpfully, and concisely.
Do NOT generate or output code blocks for greetings or conversational inquiries.",

        CodeIntent::Code => "\
You are Loomis, an expert terminal-based local code assistant.
You help the user by writing clean code that follows the style, patterns, and conventions found in the retrieved repository snippets.

STRICT INSTRUCTIONS:
1. Ground your implementation directly in the retrieved code snippets.
2. Provide a single, focused code solution answering the user's inquiry.
3. Only import libraries and call functions that are strictly necessary. Never add dead or unused imports.
4. In your explanation, describe ONLY the code you actually wrote. Do NOT list or discuss functions, libraries, or modules not present in your code.
5. Explicitly cite which retrieved snippet file path and symbol your code adapts.
6. Keep your response direct, clean, and concise.",
    };

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
                    "\n[Snippet {}] File: {} | Symbol: {} | Lang: {}\n```{}\n{}\n```\n",
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
