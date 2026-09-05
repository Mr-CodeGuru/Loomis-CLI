// examples/checkDuplicateColumns.rs
// Verifies whether id == chunk_id and content_hash == _content_hash across
// all 70,163 rows of dbe/embeddings.parquet. Rust version — parquet reading is Rust's job
// in LoomisCLI architecture.

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::path::PathBuf;
use arrow::array::{Array, StringArray};

fn main() -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let path: PathBuf = root.join("dbe").join("embeddings.parquet");
    println!("Scanning: {}\n", path.display());

    let file = File::open(&path)?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    let mut reader = builder.with_batch_size(8192).build()?;

    let mut total_rows = 0usize;
    let mut id_mismatches = 0usize;
    let mut hash_mismatches = 0usize;

    while let Some(Ok(batch)) = reader.next() {
        let chunk_id_col = batch.column_by_name("chunk_id")
            .expect("chunk_id column missing")
            .as_any().downcast_ref::<StringArray>().unwrap();
        let id_col = batch.column_by_name("id")
            .expect("id column missing")
            .as_any().downcast_ref::<StringArray>().unwrap();
        let hash_col = batch.column_by_name("content_hash")
            .expect("content_hash column missing")
            .as_any().downcast_ref::<StringArray>().unwrap();
        let underscore_hash_col = batch.column_by_name("_content_hash")
            .expect("_content_hash column missing")
            .as_any().downcast_ref::<StringArray>().unwrap();

        for i in 0..batch.num_rows() {
            total_rows += 1;
            if chunk_id_col.value(i) != id_col.value(i) {
                id_mismatches += 1;
            }
            if hash_col.value(i) != underscore_hash_col.value(i) {
                hash_mismatches += 1;
            }
        }
    }

    println!("Total rows scanned: {}", total_rows);
    println!("id vs chunk_id mismatches: {}", id_mismatches);
    println!("content_hash vs _content_hash mismatches: {}", hash_mismatches);

    if id_mismatches == 0 && hash_mismatches == 0 {
        println!("\nPASS: id == chunk_id and content_hash == _content_hash across all rows. Both are safe to collapse.");
    } else {
        println!("\nFAIL: differences detected, see above.");
    }

    Ok(())
}
