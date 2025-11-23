use crate::models::error::{BackupError, Result};
use blake2::{Blake2b512, Digest};
use std::fs;
use std::io::{BufReader, Read};
use std::path::PathBuf;

pub fn hash_file(file: &PathBuf, max_mebibytes_bytes: &usize) -> Result<String> {
    let max_bytes = max_mebibytes_bytes * 1048576;
    let reader = BufReader::new(fs::File::open(file).map_err(|cause| {
        BackupError::HashError {
            path: file.clone(),
            cause,
        }
    })?);

    hasher(reader, max_bytes).map_err(|cause| {
        BackupError::HashError {
            path: file.clone(),
            cause,
        }
    })
}

fn hasher<R: Read>(mut reader: BufReader<R>, max_bytes: usize) -> std::io::Result<String> {
    let mut hasher = Blake2b512::new();
    let mut buffer = [0; 8192];
    let mut bytes_read = 0;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes_read += count;
        hasher.update(&buffer[..count]);
        if bytes_read >= max_bytes {
            break;
        }
    }
    let output = hasher.finalize();
    Ok(hex::encode(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hash_small_file_produces_hex_output() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Hello, World!").unwrap();
        temp_file.flush().unwrap();

        let hash = hash_file(&temp_file.path().to_path_buf(), &1).unwrap();

        // BLAKE2b512 produces 128 hex characters (64 bytes * 2)
        assert_eq!(hash.len(), 128);
        // Verify it's valid hex
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_large_file_partial_hashing() {
        let mut temp_file = NamedTempFile::new().unwrap();
        // Write 2MB of data
        let data = vec![0xAB; 2 * 1024 * 1024];
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();

        // Hash with max 1 MiB limit
        let hash_1mb = hash_file(&temp_file.path().to_path_buf(), &1).unwrap();

        // Hash the same file with max 2 MiB limit
        let hash_2mb = hash_file(&temp_file.path().to_path_buf(), &2).unwrap();

        // These should be different since we're hashing different amounts
        assert_ne!(hash_1mb, hash_2mb);
    }

    #[test]
    fn test_identical_files_produce_identical_hashes() {
        let mut temp_file1 = NamedTempFile::new().unwrap();
        let mut temp_file2 = NamedTempFile::new().unwrap();
        let content = b"Identical content for testing";

        temp_file1.write_all(content).unwrap();
        temp_file1.flush().unwrap();
        temp_file2.write_all(content).unwrap();
        temp_file2.flush().unwrap();

        let hash1 = hash_file(&temp_file1.path().to_path_buf(), &1).unwrap();
        let hash2 = hash_file(&temp_file2.path().to_path_buf(), &1).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_files_produce_different_hashes() {
        let mut temp_file1 = NamedTempFile::new().unwrap();
        let mut temp_file2 = NamedTempFile::new().unwrap();

        temp_file1.write_all(b"Content A").unwrap();
        temp_file1.flush().unwrap();
        temp_file2.write_all(b"Content B").unwrap();
        temp_file2.flush().unwrap();

        let hash1 = hash_file(&temp_file1.path().to_path_buf(), &1).unwrap();
        let hash2 = hash_file(&temp_file2.path().to_path_buf(), &1).unwrap();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_empty_file_hashing() {
        let temp_file = NamedTempFile::new().unwrap();
        // Don't write anything - empty file

        let hash = hash_file(&temp_file.path().to_path_buf(), &1).unwrap();

        // Should still produce a valid hash
        assert_eq!(hash.len(), 128);
        // BLAKE2b512 of empty string is a known value
        assert_eq!(
            hash,
            "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce"
        );
    }

    #[test]
    fn test_error_on_nonexistent_file() {
        let nonexistent_path = PathBuf::from("/this/path/does/not/exist/file.txt");

        let result = hash_file(&nonexistent_path, &1);

        assert!(result.is_err());
        match result {
            Err(crate::models::error::BackupError::HashError { path, .. }) => {
                assert_eq!(path, nonexistent_path);
            }
            _ => panic!("Expected HashError"),
        }
    }
}
