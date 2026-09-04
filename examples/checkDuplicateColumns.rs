// examples/checkDuplicateColumns.rs
// Verifies whether chunk_id/id and content_hash/_content_hash are truly identical across
// all 70,163 rows of db/embeddings.parquet. Rust version — parquet reading is Rust's job
// per the locked architecture, this replaces the earlier Python one-off diagnostic.
//
// Usage: cargo run --example checkDuplicateColumns

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use lancedb::arrow::arrow_array::{Array, StringArray};
use std::fs::File;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let path: PathBuf = std::env::current_dir()?.join("db").join("embeddings.parquet");
    println!("Reading: {}\n", path.display());

    let file = File::open(&path)?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;

    let mut total_rows: usize = 0;
    let mut chunk_id_id_mismatches: usize = 0;
    let mut content_hash_mismatches: usize = 0;

    for batch_result in reader {
        let batch = batch_result?;
        total_rows += batch.num_rows();

        let chunk_id = batch
            .column_by_name("chunk_id")
            .expect("missing chunk_id column")
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("chunk_id not a string array");
        let id_col = batch
            .column_by_name("id")
            .expect("missing id column")
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("id not a string array");
        let content_hash = batch
            .column_by_name("content_hash")
            .expect("missing content_hash column")
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("content_hash not a string array");
        let underscore_content_hash = batch
            .column_by_name("_content_hash")
            .expect("missing _content_hash column")
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("_content_hash not a string array");

        for i in 0..batch.num_rows() {
            if chunk_id.value(i) != id_col.value(i) {
                chunk_id_id_mismatches += 1;
            }
            if content_hash.value(i) != underscore_content_hash.value(i) {
                content_hash_mismatches += 1;
            }
        }
    }

    println!("Total rows: {total_rows}\n");

    println!("=== chunk_id vs id ===");
    if chunk_id_id_mismatches == 0 {
        println!("IDENTICAL across all {total_rows} rows.");
    } else {
        println!("{chunk_id_id_mismatches} mismatches out of {total_rows} rows — NOT identical.");
    }

    println!("\n=== content_hash vs _content_hash ===");
    if content_hash_mismatches == 0 {
        println!("IDENTICAL across all {total_rows} rows.");
    } else {
        println!("{content_hash_mismatches} mismatches out of {total_rows} rows — NOT identical.");
    }

    // Print a few sample rows so the difference can actually be inspected, not just counted.
    println!("\n=== Sample rows: content_hash vs _content_hash ===");
    let file2 = File::open(&path)?;
    let reader2 = ParquetRecordBatchReaderBuilder::try_new(file2)?.build()?;
    let mut shown = 0;
    'outer: for batch_result in reader2 {
        let batch = batch_result?;
        let content_hash = batch
            .column_by_name("content_hash").unwrap()
            .as_any().downcast_ref::<StringArray>().unwrap();
        let underscore_content_hash = batch
            .column_by_name("_content_hash").unwrap()
            .as_any().downcast_ref::<StringArray>().unwrap();
        for i in 0..batch.num_rows() {
            println!("  content_hash:  {}", content_hash.value(i));
            println!("  _content_hash: {}", underscore_content_hash.value(i));
            println!();
            shown += 1;
            if shown >= 3 {
                break 'outer;
            }
        }
    }

    Ok(())
}