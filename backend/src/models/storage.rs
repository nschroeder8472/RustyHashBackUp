use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct StorageStats {
    pub total_source_files: u64,
    pub total_source_size: u64,
    pub destination_stats: Vec<DestinationStorageStats>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DestinationStorageStats {
    pub destination_root: String,
    pub file_count: u64,
    pub total_size: u64,
}
