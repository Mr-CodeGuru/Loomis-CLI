use anyhow::{bail, Result};
use futures_util::TryStreamExt;
use lancedb::arrow::arrow_array::{Array, Float32Array, RecordBatchIterator, RecordBatchReader, StringArray};
use lancedb::query::{ExecutableQuery, QueryBase};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk_id: String,
    pub path: String,
    pub language: String,
    pub extracted_name: String,
    pub text: String,
    pub distance: f32,
}

pub struct VectorStore {
    table: lancedb::Table,
}

impl VectorStore {
    pub fn resolve_paths() -> (PathBuf, PathBuf) {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let base_dir = if root.join("dbe").exists() {
            root.join("dbe")
        } else {
            root.join("db")
        };
        let db_path = base_dir.join("lancedb");
        let parquet_path = base_dir.join("embeddings.parquet");
        (db_path, parquet_path)
    }

    pub async fn connect_or_create() -> Result<Self> {
        let (db_path, parquet_path) = Self::resolve_paths();
        std::fs::create_dir_all(&db_path)?;

        let db = lancedb::connect(db_path.to_str().unwrap()).execute().await?;
        let table_names = db.table_names().execute().await?;

        let table = if table_names.contains(&"chunks".to_string()) {
            println!("Connecting to existing LanceDB 'chunks' table at {}", db_path.display());
            db.open_table("chunks").execute().await?
        } else {
            println!("Creating LanceDB 'chunks' table from {}", parquet_path.display());
            if !parquet_path.exists() {
                bail!("Parquet file not found at: {}", parquet_path.display());
            }

            let file = File::open(&parquet_path)?;
            let reader_builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
            let schema = reader_builder.schema().clone();
            let reader = reader_builder.build()?;
            let batches: Vec<_> = reader.collect::<std::result::Result<Vec<_>, _>>()?;

            let batch_iter = RecordBatchIterator::new(batches.into_iter().map(Ok), schema);
            let boxed_reader: Box<dyn RecordBatchReader + Send> = Box::new(batch_iter);

            db.create_table("chunks", boxed_reader).execute().await?
        };

        Ok(Self { table })
    }

    pub async fn count_rows(&self) -> Result<usize> {
        self.table.count_rows(None).await.map_err(Into::into)
    }

    pub async fn search(&self, query_vector: Vec<f32>, limit: usize) -> Result<Vec<SearchResult>> {
        let batches = self
            .table
            .vector_search(query_vector)?
            .limit(limit)
            .execute()
            .await?
            .try_collect::<Vec<_>>()
            .await?;

        let mut results = Vec::new();

        for batch in batches {
            let num_rows = batch.num_rows();

            let chunk_id_col = batch
                .column_by_name("chunk_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let path_col = batch
                .column_by_name("path")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let lang_col = batch
                .column_by_name("language")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let name_col = batch
                .column_by_name("extracted_name")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let text_col = batch
                .column_by_name("text")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let dist_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            for i in 0..num_rows {
                let chunk_id = chunk_id_col.map(|c| c.value(i).to_string()).unwrap_or_default();
                let path = path_col.map(|c| c.value(i).to_string()).unwrap_or_default();
                let language = lang_col.map(|c| c.value(i).to_string()).unwrap_or_default();
                let extracted_name = name_col.map(|c| c.value(i).to_string()).unwrap_or_default();
                let text = text_col.map(|c| c.value(i).to_string()).unwrap_or_default();
                let distance = dist_col.map(|c| c.value(i)).unwrap_or(0.0);

                results.push(SearchResult {
                    chunk_id,
                    path,
                    language,
                    extracted_name,
                    text,
                    distance,
                });
            }
        }

        Ok(results)
    }
}
