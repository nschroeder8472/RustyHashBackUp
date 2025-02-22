use crate::models::source_row::SourceRow;
use blake2::{Blake2b512, Digest};
use std::fs;
use std::io::{BufReader, Error, Read};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

pub fn hash_file(file: &PathBuf, max_bytes: usize) -> SourceRow {
    let file_path = file.parent().unwrap().to_str().unwrap();
    let file_name = file.file_name().unwrap().to_str().unwrap();
    println!("Hashing {}", file_name);
    let reader = BufReader::new(fs::File::open(&file).unwrap());
    match hasher(reader, max_bytes) {
        Ok(hash) => {
            let source_row = SourceRow {
                id: 0,
                file_name: String::from(file_name),
                file_path: String::from(file_path),
                hash,
                last_modified: file
                    .metadata()
                    .unwrap()
                    .modified()
                    .unwrap()
                    .duration_since(UNIX_EPOCH)
                    .expect("File last_modified is older than Epoch 0"),
            };
            source_row
        }
        Err(_) => {
            panic!("Failed to hash file");
        }
    }
}

fn hasher<R: Read>(mut reader: BufReader<R>, max_bytes: usize) -> rusqlite::Result<String, Error> {
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
        if bytes_read > max_bytes {
            break;
        }
    }
    let output = hasher.finalize();
    let result = String::from_utf8_lossy(&output);
    Ok(result.parse().unwrap())
}
