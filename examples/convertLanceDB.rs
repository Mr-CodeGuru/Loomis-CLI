// examples/convertLanceDB.rs
// Creates a real LanceDB table from dbe/embeddings.parquet and runs a vector search against it —
// all within Rust. Confirms the arrow/parquet crate versions pinned in Cargo.toml match lancedb's
// internal arrow types without any mismatch errors.

use arrow::array::{RecordBatchIterator, RecordBatchReader};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== LanceDB Parquet Conversion & Search Example ===\n");

    let root = std::env::current_dir()?;
    let base_dir = if root.join("dbe").exists() {
        root.join("dbe")
    } else {
        root.join("db")
    };
    let parquet_path: PathBuf = base_dir.join("embeddings.parquet");
    let db_path: PathBuf = base_dir.join("lancedb");

    if !parquet_path.exists() {
        anyhow::bail!("Parquet file not found at: {}", parquet_path.display());
    }

    println!("1. Reading schema and batches from: {}", parquet_path.display());
    let file = File::open(&parquet_path)?;
    let reader_builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    let schema = reader_builder.schema().clone();
    let reader = reader_builder.build()?;

    let batches: Vec<_> = reader.collect::<std::result::Result<Vec<_>, _>>()?;
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    println!("   Loaded {} batches, total rows: {}", batches.len(), total_rows);

    let sample_vector: Vec<f32> = {
        use arrow::array::{Array, FixedSizeListArray, Float32Array};
        let first_batch = &batches[0];
        let vec_col = first_batch
            .column_by_name("vector")
            .expect("vector column missing");
        let fixed_list = vec_col
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .expect("downcast to FixedSizeListArray failed");
        let values = fixed_list.values();
        let float_array = values
            .as_any()
            .downcast_ref::<Float32Array>()
            .expect("downcast to Float32Array failed");
        let dim = fixed_list.value_length() as usize;
        (0..dim).map(|i| float_array.value(i)).collect()
    };
    println!("   Extracted sample query vector (dimension: {})", sample_vector.len());

    println!("\n2. Initializing LanceDB at: {}", db_path.display());
    std::fs::create_dir_all(&db_path)?;
    let db = lancedb::connect(db_path.to_str().unwrap()).execute().await?;

    let table_names = db.table_names().execute().await?;
    println!("   Existing tables: {:?}", table_names);

    let table = if table_names.contains(&"chunks".to_string()) {
        println!("   Table 'chunks' already exists. Opening it...");
        db.open_table("chunks").execute().await?
    } else {
        println!("   Creating table 'chunks' from RecordBatches...");
        let batch_iter = RecordBatchIterator::new(batches.into_iter().map(Ok), schema);
        let boxed_reader: Box<dyn RecordBatchReader + Send> = Box::new(batch_iter);
        db.create_table("chunks", boxed_reader).execute().await?
    };

    println!("\n3. Running vector search against LanceDB table...");
    let results = table
        .vector_search(sample_vector)?
        .limit(5)
        .execute()
        .await?
        .try_collect::<Vec<_>>()
        .await?;

    println!("   Received {} result batch(es)", results.len());
    for (i, batch) in results.iter().enumerate() {
        println!("   Batch {} rows: {}", i, batch.num_rows());
    }

    println!("\nPASS: LanceDB successfully created and queried from parquet without Arrow type conflicts!");
    Ok(())
}
