// examples/convertLanceDB.rs
// Creates a real LanceDB table from db/embeddings.parquet and runs a vector search against it —
// the first actual LanceDB usage in this project (everything before this only read parquet
// schema, never built or queried a table).
//
// Usage: cargo run --example convertLanceDB
//
// NOT YET RUN. Confidence note: `lancedb`'s Rust API surface hasn't been reliable in this
// project so far (see the Cargo.toml feature-flag/version issue already hit). The general shape
// here (connect -> read parquet into Arrow RecordBatches -> create_table -> vector_search) matches
// lancedb's documented usage pattern, but exact method names/signatures for THIS installed
// version (0.38.0) are not verified against source. If this fails to compile, paste the error —
// don't assume the fix, check the actual crate docs for 0.38.0 specifically.

use futures_util::TryStreamExt;
use lancedb::arrow::arrow_array::{
    FixedSizeListArray, Float32Array, RecordBatchIterator, RecordBatchReader,
};
use lancedb::query::{ExecutableQuery, QueryBase};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let parquet_path: PathBuf = std::env::current_dir()?.join("db").join("embeddings.parquet");
    let db_path: PathBuf = std::env::current_dir()?.join("db").join("lancedb");

    println!("Reading parquet from: {}", parquet_path.display());
    let file = File::open(&parquet_path)?;
    let reader_builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    let schema = reader_builder.schema().clone();
    let reader = reader_builder.build()?;

    // Collect batches so we can both build the table and grab a sample vector for the query below.
    let batches: Vec<_> = reader.collect::<Result<Vec<_>, _>>()?;
    println!("Read {} record batch(es).", batches.len());

    // Grab the first row's vector as a stand-in query — real usage will embed a user query via
    // the Python sidecar instead, this is just to prove search works end-to-end.
    let first_batch = batches.first().expect("expected at least one batch");
    let vector_col = first_batch
        .column_by_name("vector")
        .expect("expected a 'vector' column");
    let vector_array = vector_col
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .expect("expected 'vector' to be a FixedSizeListArray");
    let sample_vector: Vec<f32> = vector_array
        .value(0)
        .as_any()
        .downcast_ref::<Float32Array>()
        .expect("expected float32 values")
        .values()
        .to_vec();
    println!("Sample query vector dim: {}", sample_vector.len());

    println!("\nConnecting to LanceDB at: {}", db_path.display());
    let db = lancedb::connect(db_path.to_str().unwrap()).execute().await?;

    let table_name = "chunks";
    println!("Creating table '{table_name}' from parquet data ...");

    let batch_iter = RecordBatchIterator::new(batches.clone().into_iter().map(Ok), schema);
    let boxed_reader: Box<dyn RecordBatchReader + Send> = Box::new(batch_iter);
    let table = db
        .create_table(table_name, boxed_reader)
        .execute()
        .await?;

    println!("Table created. Running a sample vector search ...");

    let results = table
        .vector_search(sample_vector)?
        .limit(5)
        .execute()
        .await?
        .try_collect::<Vec<_>>()
        .await?;

    println!("\n=== Search results ===");
    println!("Returned {} batch(es) of results.", results.len());
    for batch in &results {
        println!("  Batch with {} row(s), {} column(s).", batch.num_rows(), batch.num_columns());
    }

    if !results.is_empty() {
        println!("\nPASS: LanceDB table created and vector search returned results.");
    } else {
        println!("\nFAIL: vector search returned no results.");
    }

    Ok(())
}