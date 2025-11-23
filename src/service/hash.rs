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
