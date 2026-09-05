// examples/testParquet.rs
// Rust-only equivalent of evals/inspectParquet.py — reads dbe/embeddings.parquet's
// schema and first row using the parquet crate, without Python.

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::reader::{FileReader, SerializedFileReader};
use std::fs::File;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let path: PathBuf = root.join("dbe").join("embeddings.parquet");
    println!("Reading parquet from: {}\n", path.display());

    let file = File::open(&path)?;
    let reader = SerializedFileReader::new(file)?;
    let file_metadata = reader.metadata().file_metadata();

    println!("=== Schema (Parquet format) ===");
    for (i, col) in file_metadata.schema().get_fields().iter().enumerate() {
        println!("  [{}] {}: {:?}", i, col.name(), col.get_basic_info().repetition());
    }

    println!("\n=== Row Count ===");
    println!("  Total rows: {}", file_metadata.num_rows());
    println!("  Row groups: {}", reader.metadata().num_row_groups());

    let file = File::open(&path)?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    let arrow_schema = builder.schema().clone();

    println!("\n=== Schema (Arrow format) ===");
    for field in arrow_schema.fields() {
        println!("  {}: {}", field.name(), field.data_type());
    }

    let mut reader = builder.with_batch_size(1).build()?;
    if let Some(batch) = reader.next() {
        let batch = batch?;
        println!("\n=== Sample row (first row, Arrow RecordBatch) ===");
        for (i, field) in arrow_schema.fields().iter().enumerate() {
            let col = batch.column(i);
            println!("  {}: len={}, null_count={}", field.name(), col.len(), col.null_count());
        }
    }

    println!("\nPASS: parquet schema and sample row read successfully in Rust.");
    Ok(())
}
