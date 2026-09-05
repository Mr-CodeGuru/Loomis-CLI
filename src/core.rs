use anyhow::Result;
use crate::config::AppConfig;
use crate::db::{SearchResult, VectorStore};
use crate::llm::{build_rag_messages, fallback_classify_code_intent, ChatMessage, CodeIntent, LlmClient};
use crate::sidecar::SidecarClient;

/// Core engine managing state and orchestrating intent classification,
/// embedding sidecar IPC, LanceDB retrieval, and conversation history.
pub struct LoomisCore {
    pub config: AppConfig,
    pub vector_store: VectorStore,
    pub sidecar: SidecarClient,
    pub llm: LlmClient,
    pub history: Vec<ChatMessage>,
}

/// Holds the results of intent classification, retrieved chunks, and prompt messages.
pub struct QueryPreparation {
    pub intent: CodeIntent,
    pub chunks: Vec<SearchResult>,
    pub messages: Vec<ChatMessage>,
}

impl LoomisCore {
    /// Initialize vector store, Python embedding sidecar, and LLM client.
    pub async fn init(config: AppConfig) -> Result<Self> {
        let vector_store = VectorStore::connect_or_create().await?;
        let sidecar = SidecarClient::new().await?;
        let llm = LlmClient::new(
            config.endpoint_url.clone(),
            config.model.clone(),
            config.api_key.clone(),
        );

        Ok(Self {
            config,
            vector_store,
            sidecar,
            llm,
            history: Vec::new(),
        })
    }

    /// Classify user query intent and conditionally perform vector retrieval.
    pub async fn prepare_query(&mut self, query: &str) -> Result<QueryPreparation> {
        let intent = match self.llm.classify_code_intent(query).await {
            Ok(i) => i,
            Err(_) => fallback_classify_code_intent(query),
        };

        let chunks = match intent {
            CodeIntent::Chat => Vec::new(),
            CodeIntent::Code => {
                let query_vector = self.sidecar.embed(query).await?;
                self.vector_store.search(query_vector, self.config.top_k).await?
            }
        };

        let messages = build_rag_messages(query, &chunks, &self.history, intent);
        Ok(QueryPreparation {
            intent,
            chunks,
            messages,
        })
    }

    /// Record turn into ephemeral in-memory conversation history.
    pub fn record_turn(&mut self, query: &str, response: &str) {
        if !response.trim().starts_with("I can't answer that") {
            self.history.push(ChatMessage {
                role: "user".to_string(),
                content: query.to_string(),
            });
            self.history.push(ChatMessage {
                role: "assistant".to_string(),
                content: response.to_string(),
            });
        }
    }
}
