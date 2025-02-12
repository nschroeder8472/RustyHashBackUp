use blake2::{Blake2b512, Digest};
use clap::{arg, Parser};
use chrono::{NaiveDateTime, Utc};
use std::io::{BufReader, Error, Read};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rayon::prelude::*;
use walkdir::WalkDir;

fn main() {
    let args = Cli::parse();
    let max_mebibytes = args.number_of_mebibytes * 1048576;

    let source_files = get_files_in_path(args.source_path);
    let destination_files = get_files_in_path(args.destination_path);

    if source_files.is_empty() {
        println!("No files found");
        return;
    }

    let source_hash_data = hash_files(source_files, max_mebibytes);
    let mut destination_hash_data= Vec::new();
    if !destination_files.is_empty() {
        destination_hash_data = hash_files(destination_files, max_mebibytes);
    }

    if source_hash_data.len() != destination_hash_data.len() {
        println!("Source and destination hash data do not match");
    } else {
        for i in 0..source_hash_data.len() {
            let hash_compare = source_hash_data[i].hash == destination_hash_data[i].hash;
            println!("Hashes are equal: {}", hash_compare)
        }
    }

    println!("Done");
}

fn get_files_in_path(top_directory: String) -> Vec<PathBuf> {
    let mut files= Vec::new();
    for entry in WalkDir::new(top_directory)
        .follow_links(true)
        .contents_first(true)
        .into_iter()
        .filter_map(Result::ok) {
        if entry.file_type().is_dir() {
            continue;
        }
        files.push(entry.path().to_path_buf());
    };
    files
}

fn hash_files(files: Vec<PathBuf>, max_mebibytes: usize) -> Vec<FileHash> {
    let result = Arc::new(Mutex::new(Vec::new()));

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

    Arc::try_unwrap(result).unwrap().into_inner().unwrap()
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
    #[arg(short = 's', long = "source")]
    source_path: String,
    #[arg(short = 'd', long = "destination")]
    destination_path: String,
    #[arg(short = 'r', long = "relative_path", default_value = "")]
    relative_path: String,
    #[arg(short = 'm', long = "max_size", default_value = "1")]
    number_of_mebibytes: usize
}

#[derive(Debug)]
struct FileHash {
    relative_path: String,
    hash: String,
    date: NaiveDateTime
}
