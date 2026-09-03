// examples/test_embed_jina.rs
// Verifies candle-transformers' JinaBert can load jina-embeddings-v2-base-code and produce
// a 768-dim embedding — the Rust-only equivalent of loadModels/loadJina.py.
//
// NOT YET RUN — this is a best-effort port based on candle-transformers' public jina_bert
// module and the known model config (768 hidden size, ALiBi). Config field names and the
// exact API surface may not match current candle-transformers exactly — verify against
// the actual crate docs/source if this fails to compile, don't assume this is correct as-is.
//
// Usage: cargo run --example test_embed_jina --release

use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::jina_bert::{BertModel, Config};
use hf_hub::api::sync::Api;
use tokenizers::Tokenizer;

fn main() -> anyhow::Result<()> {
    let device = Device::Cpu;

    let api = Api::new()?;
    let repo = api.model("jinaai/jina-embeddings-v2-base-code".to_string());

    let config_path = repo.get("config.json")?;
    let tokenizer_path = repo.get("tokenizer.json")?;
    let weights_path = repo.get("model.safetensors")?;

    let config: Config = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(anyhow::Error::msg)?;

    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F32, &device)?
    };
    let model = BertModel::new(vb, &config)?;

    let sample_text = "def add(a, b):\n    return a + b";
    let encoding = tokenizer.encode(sample_text, true).map_err(anyhow::Error::msg)?;
    let token_ids = Tensor::new(encoding.get_ids(), &device)?.unsqueeze(0)?;

    let embeddings = model.forward(&token_ids)?;
    // Mean-pool over the sequence dimension — standard for sentence embeddings.
    let pooled = embeddings.mean(1)?;

    println!("Embedding shape: {:?}", pooled.shape());
    let dim = pooled.shape().dims()[pooled.shape().dims().len() - 1];
    if dim == 768 {
        println!("PASS: embedding dimension matches expected 768.");
    } else {
        println!("FAIL: expected dimension 768, got {dim}.");
    }

    Ok(())
}