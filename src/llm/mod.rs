pub mod client;
pub mod prompt;

pub use client::{ChatMessage, LlmClient};
pub use prompt::{build_rag_messages, fallback_classify_code_intent, is_explicit_code_request, CodeIntent};
