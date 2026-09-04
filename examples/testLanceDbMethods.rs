// examples/testLanceDbMethods.rs
use loomiscli::db::VectorStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = VectorStore::connect_or_create().await?;
    let total = store.count_rows().await?;
    println!("Total rows in 'chunks': {}", total);
    Ok(())
}
