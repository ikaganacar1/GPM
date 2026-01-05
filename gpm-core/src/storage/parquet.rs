use crate::error::{GpmError, Result};
use polars::prelude::*;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub struct ParquetArchiver {
    archive_dir: PathBuf,
}

impl ParquetArchiver {
    pub fn new<P: AsRef<Path>>(archive_dir: P) -> Result<Self> {
        let archive_dir = archive_dir.as_ref().to_path_buf();

        std::fs::create_dir_all(&archive_dir)?;

        Ok(Self { archive_dir })
    }

    pub async fn archive_gpu_metrics(
        &self,
        db_path: &Path,
        cutoff_date: chrono::NaiveDate,
    ) -> Result<usize> {
        let query = format!(
            "SELECT * FROM gpu_metrics WHERE DATE(timestamp) < '{}'",
            cutoff_date
        );

        self.archive_table(db_path, "gpu_metrics", &query, cutoff_date)
            .await
    }

    pub async fn archive_process_events(
        &self,
        db_path: &Path,
        cutoff_date: chrono::NaiveDate,
    ) -> Result<usize> {
        let query = format!(
            "SELECT * FROM process_events WHERE DATE(timestamp) < '{}'",
            cutoff_date
        );

        self.archive_table(db_path, "process_events", &query, cutoff_date)
            .await
    }

    pub async fn archive_llm_sessions(
        &self,
        db_path: &Path,
        cutoff_date: chrono::NaiveDate,
    ) -> Result<usize> {
        let query = format!(
            "SELECT * FROM llm_sessions WHERE DATE(start_time) < '{}'",
            cutoff_date
        );

        self.archive_table(db_path, "llm_sessions", &query, cutoff_date)
            .await
    }

    async fn archive_table(
        &self,
        db_path: &Path,
        table_name: &str,
        query: &str,
        date: chrono::NaiveDate,
    ) -> Result<usize> {
        let df = self.read_from_sqlite(db_path, query)?;

        if df.height() == 0 {
            info!("No data to archive for table {} before {}", table_name, date);
            return Ok(0);
        }

        let parquet_file = self
            .archive_dir
            .join(format!("{}_{}.parquet", table_name, date));

        self.write_parquet(&df, &parquet_file)?;

        info!(
            "Archived {} records from {} to {}",
            df.height(),
            table_name,
            parquet_file.display()
        );

        Ok(df.height())
    }

    fn read_from_sqlite(&self, _db_path: &Path, _query: &str) -> Result<DataFrame> {
        warn!("Parquet archival from SQLite not yet implemented - using placeholder");

        let df = df! {
            "placeholder" => &[0i64],
        }
        .map_err(|e| GpmError::ParquetError(format!("Failed to create DataFrame: {}", e)))?;

        Ok(df)
    }

    fn write_parquet(&self, df: &DataFrame, path: &Path) -> Result<()> {
        let file = std::fs::File::create(path)?;

        ParquetWriter::new(file)
            .with_compression(ParquetCompression::Snappy)
            .finish(&mut df.clone())
            .map_err(|e| GpmError::ParquetError(format!("Failed to write Parquet: {}", e)))?;

        Ok(())
    }

    pub fn read_parquet(&self, path: &Path) -> Result<DataFrame> {
        let file = std::fs::File::open(path)?;

        let df = ParquetReader::new(file)
            .finish()
            .map_err(|e| GpmError::ParquetError(format!("Failed to read Parquet: {}", e)))?;

        Ok(df)
    }

    pub fn list_archives(&self) -> Result<Vec<PathBuf>> {
        let mut archives = Vec::new();

        for entry in std::fs::read_dir(&self.archive_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("parquet") {
                archives.push(path);
            }
        }

        archives.sort();
        Ok(archives)
    }

    pub fn get_archive_size_bytes(&self) -> Result<u64> {
        let mut total_size = 0u64;

        for entry in std::fs::read_dir(&self.archive_dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            if metadata.is_file() {
                total_size += metadata.len();
            }
        }

        Ok(total_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parquet_round_trip() {
        let dir = tempdir().unwrap();
        let archiver = ParquetArchiver::new(dir.path()).unwrap();

        let df = df! {
            "id" => &[1i64, 2, 3],
            "name" => &["Alice", "Bob", "Charlie"],
            "value" => &[10.5, 20.3, 30.1],
        }
        .unwrap();

        let parquet_path = dir.path().join("test.parquet");
        archiver.write_parquet(&df, &parquet_path).unwrap();

        let df_read = archiver.read_parquet(&parquet_path).unwrap();

        assert_eq!(df.shape(), df_read.shape());
    }
}
