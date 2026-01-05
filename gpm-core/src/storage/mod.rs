pub mod db;
pub mod parquet;

pub use db::Database;
pub use parquet::ParquetArchiver;

use crate::config::GpmConfig;
use crate::error::Result;
use tracing::info;

pub struct StorageManager {
    pub database: Database,
    pub archiver: ParquetArchiver,
    retention_days: i64,
}

impl StorageManager {
    pub async fn new(config: &GpmConfig) -> Result<Self> {
        let db_path = config.database_path();
        let archive_dir = &config.storage.archive_dir;

        let database = Database::new(&db_path).await?;
        let archiver = ParquetArchiver::new(archive_dir)?;

        info!("Storage manager initialized");
        info!("  Database: {}", db_path.display());
        info!("  Archive: {}", archive_dir.display());

        Ok(Self {
            database,
            archiver,
            retention_days: config.storage.retention_days as i64,
        })
    }

    pub async fn perform_maintenance(&self, config: &GpmConfig) -> Result<()> {
        if !config.storage.enable_parquet_archival {
            return Ok(());
        }

        let cutoff_date = (chrono::Utc::now() - chrono::Duration::days(self.retention_days))
            .date_naive();

        info!("Running storage maintenance (archiving data before {})", cutoff_date);

        let db_path = config.database_path();

        let gpu_count = self
            .archiver
            .archive_gpu_metrics(&db_path, cutoff_date)
            .await?;

        let process_count = self
            .archiver
            .archive_process_events(&db_path, cutoff_date)
            .await?;

        let llm_count = self
            .archiver
            .archive_llm_sessions(&db_path, cutoff_date)
            .await?;

        if gpu_count + process_count + llm_count > 0 {
            self.database
                .cleanup_old_data(self.retention_days)
                .await?;

            info!(
                "Archived {} GPU metrics, {} process events, {} LLM sessions",
                gpu_count, process_count, llm_count
            );
        }

        let archive_size = self.archiver.get_archive_size_bytes()?;
        info!("Archive directory size: {:.2} MB", archive_size as f64 / 1024.0 / 1024.0);

        Ok(())
    }
}
