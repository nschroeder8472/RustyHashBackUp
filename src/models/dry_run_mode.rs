/// Defines the dry-run behavior mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DryRunMode {
    /// Normal operation - actually copy files and update database
    None,

    /// Quick preview - skip expensive operations like hashing
    /// Shows what would be processed based on last modified dates only
    Quick,

    /// Full simulation - perform all checks and hashing but skip file copy and database writes
    /// Shows exactly what would happen in a real backup
    Full,
}

impl DryRunMode {
    /// Returns true if this is any dry-run mode (Quick or Full)
    pub fn is_dry_run(&self) -> bool {
        matches!(self, DryRunMode::Quick | DryRunMode::Full)
    }

    /// Returns true if this is Quick mode (skip hashing)
    pub fn is_quick(&self) -> bool {
        matches!(self, DryRunMode::Quick)
    }

    /// Returns true if this is Full mode (do hashing)
    pub fn is_full(&self) -> bool {
        matches!(self, DryRunMode::Full)
    }

    /// Returns true if hashing should be performed
    pub fn should_hash(&self) -> bool {
        !matches!(self, DryRunMode::Quick)
    }

    /// Returns true if files should actually be copied
    pub fn should_copy_files(&self) -> bool {
        matches!(self, DryRunMode::None)
    }

    /// Returns true if database should be updated
    pub fn should_update_database(&self) -> bool {
        matches!(self, DryRunMode::None)
    }

    /// Get display string for progress bars
    pub fn progress_prefix(&self) -> &'static str {
        match self {
            DryRunMode::None => "",
            DryRunMode::Quick => "[DRY RUN - QUICK] ",
            DryRunMode::Full => "[DRY RUN - FULL] ",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_dry_run() {
        assert!(!DryRunMode::None.is_dry_run());
        assert!(DryRunMode::Quick.is_dry_run());
        assert!(DryRunMode::Full.is_dry_run());
    }

    #[test]
    fn test_should_hash() {
        assert!(DryRunMode::None.should_hash());
        assert!(!DryRunMode::Quick.should_hash());
        assert!(DryRunMode::Full.should_hash());
    }

    #[test]
    fn test_should_copy_files() {
        assert!(DryRunMode::None.should_copy_files());
        assert!(!DryRunMode::Quick.should_copy_files());
        assert!(!DryRunMode::Full.should_copy_files());
    }

    #[test]
    fn test_should_update_database() {
        assert!(DryRunMode::None.should_update_database());
        assert!(!DryRunMode::Quick.should_update_database());
        assert!(!DryRunMode::Full.should_update_database());
    }
}
