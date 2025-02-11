use blake2::{Blake2b512, Digest};
use clap::{arg, Parser};
use chrono::{NaiveDateTime, Utc};
use std::io::{BufReader, Error, Read};
use std::fs;
use std::sync::{Arc, Mutex};
use rayon::prelude::*;
use walkdir::WalkDir;

fn main() {
    let args = Cli::parse();
    let max_mebibytes = args.number_of_mebibytes * 1048576;
    let mut files= Vec::new();
    for entry in WalkDir::new(args.input_path)
        .follow_links(true)
        .contents_first(true)
        .into_iter()
        .filter_map(Result::ok) {
        if entry.file_type().is_dir() {
            continue;
        }
        files.push(entry.path().to_path_buf());
    };

    if files.is_empty() {
        println!("No files found");
        return;
    }

    let mut result = Arc::new(Mutex::new(Vec::new()));

    files.par_iter().for_each(|file| {
        let file_path = file.to_str().unwrap();
        println!("Hashing {}", file_path);
        let reader = BufReader::new(fs::File::open(file).unwrap());
        let hash = hasher(reader, max_mebibytes);
        match hash {
            Ok(hash) => {
                let file_hash = FileHash {
                    relative_path: String::from(file_path),
                    hash,
                    date: Utc::now().naive_utc()
                };
                result.lock().unwrap().push(file_hash);
            }
            Err(_) => {panic!("Failed to hash file");}
        }
    });
    for hash in result.lock().unwrap().iter() {
        println!("path:{}, hash:{}, date_time:{}", hash.relative_path, hash.hash, hash.date);
    }
    println!("Done");
}

fn hasher<R: Read>(mut reader: BufReader<R>, max_mebibytes: usize) -> Result<String, Error> {
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
        if bytes_read > max_mebibytes {
            break;
        }
    }
    let output = hasher.finalize();
    let result = String::from_utf8_lossy(&output);
    Ok(result.parse().unwrap())
}

#[derive(Parser)]
struct Cli {
    #[arg(short = 'i', long = "input")]
    input_path: String,
    #[arg(short = 'r', long = "recursive", default_value = "true")]
    recursive: bool,
    #[arg(short = 'a', long = "all")]
    recalculate_all: bool,
    #[arg(short = 's', long = "size", default_value = "1")]
    number_of_mebibytes: usize
}

struct FileHash {
    relative_path: String,
    hash: String,
    date: NaiveDateTime
}
