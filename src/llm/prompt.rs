use crate::db::SearchResult;
use super::client::ChatMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryIntent {
    Greeting,
    CodeGeneration,
    GeneralInquiry,
}

pub fn classify_query_intent(query: &str) -> QueryIntent {
    let lower = query.trim().to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();

    if words.is_empty() {
        return QueryIntent::Greeting;
    }

    // 1. Common greetings and casual conversation
    let greeting_phrases = [
        "hi", "hello", "hey", "greetings", "good morning", "good afternoon",
        "good evening", "who are you", "who are you?", "what are you",
        "what are you?", "how are you", "how are you?", "what can you do",
        "what can you do?", "thanks", "thank you", "bye", "goodbye"
    ];

    if greeting_phrases.contains(&lower.as_str())
        || (words.len() <= 2 && matches!(words[0], "hi" | "hello" | "hey" | "sup" | "yo" | "hola"))
    {
        return QueryIntent::Greeting;
    }

    // 2. Explicit code generation intent (user specifically asks to write / generate / implement code)
    let code_gen_patterns = [
        "write", "implement", "generate", "code a", "create a function",
        "create a class", "create a script", "create an", "refactor",
        "extend the", "extend a", "build a function", "make a function",
        "develop a", "write a", "write code", "rewrite", "write python",
        "write rust", "draft a", "program that", "script that"
    ];

    for pattern in &code_gen_patterns {
        if lower.contains(pattern) {
            return QueryIntent::CodeGeneration;
        }
    }

    if words[0] == "write"
        || words[0] == "implement"
        || words[0] == "refactor"
        || words[0] == "generate"
        || words[0] == "extend"
    {
        return QueryIntent::CodeGeneration;
    }

    QueryIntent::GeneralInquiry
}

pub fn build_rag_messages(
    query: &str,
    context_chunks: &[SearchResult],
    history: &[ChatMessage],
) -> Vec<ChatMessage> {
    let intent = classify_query_intent(query);
    let mut messages = Vec::new();

    let system_instruction = match intent {
        QueryIntent::Greeting => "\
You are Loomis, an expert terminal-based local code assistant.
Respond to greetings and conversational inquiries politely, helpfully, and concisely.
Do NOT output code blocks or generate code for greetings or casual conversation.",

        QueryIntent::GeneralInquiry => "\
You are Loomis, an expert terminal-based local code assistant.
Answer the user's inquiry directly using the retrieved repository snippets below.
Explain how the code, patterns, or functions work and cite the relevant file paths and symbol names.
Do NOT generate a new code implementation unless the user explicitly asks you to write or implement code.
Be direct, helpful, and concise.",

        QueryIntent::CodeGeneration => "\
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
            "\n[Snippet {}] File: {} | Symbol: {} | Lang: {}\n```{}\n{}\n```\n",
            i + 1,
            chunk.path,
            symbol_label,
            chunk.language,
            chunk.language,
            snippet_text
        ));
    }

    let user_content = match intent {
        QueryIntent::Greeting => format!("User: {}", query),
        QueryIntent::GeneralInquiry => {
            if context_chunks.is_empty() {
                format!("User Question:\n{}", query)
            } else {
                format!(
                    "User Question:\n{}\n\nContext Snippets from Repository:\n{}\nInstructions:\nAnswer the user's question directly using the snippets above, citing the relevant file path and function names. Do not write an unrequested code implementation.",
                    query, context_block
                )
            }
        }
        QueryIntent::CodeGeneration => {
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
