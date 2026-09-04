// examples/inspect_parquet.rs
// Rust-only equivalent of loadModels/inspect_parquet.py — reads db/embeddings.parquet's
// schema using the `parquet` crate directly, no Python involved.
//
// NOT YET RUN — verify the `parquet` crate's current API against its docs if this doesn't
// compile as-is; API surface for reading schema/row groups has shifted across versions.
//
// Usage: cargo run --example testParquet --release

use parquet::file::reader::{FileReader, SerializedFileReader};
use std::fs::File;
use std::path::PathBuf;
 
fn main() -> anyhow::Result<()> {
    let path: PathBuf = std::env::current_dir()?.join("db").join("embeddings.parquet");
    println!("Reading schema from: {}", path.display());
 
    let file = File::open(&path)?;
    let reader = SerializedFileReader::new(file)?;
    let metadata = reader.metadata();
 
    println!("\n=== Schema ===");
    let schema = metadata.file_metadata().schema();
    schema.get_fields().iter().for_each(|field| {
        if field.is_primitive() {
            println!("  {}: {:?}", field.name(), field.get_physical_type());
        } else {
            println!("  {}: (complex/group type — {:?})", field.name(), field.get_basic_info().logical_type_ref());
        }
    });
 
    println!("\n=== Row count ===");
    println!("  {}", metadata.file_metadata().num_rows());
 
    println!("\n=== Row groups ===");
    println!("  {}", metadata.num_row_groups());
 
    Ok(())
}