use std::fs::OpenOptions;
use std::io::prelude::*;
use std::process::Command;
use std::env;
use std::fs::File;
use chrono::{DateTime, Local, NaiveDateTime, Utc};
use chrono::format::ParseError;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;
use std::thread;
use std::time::Duration;

pub const COUNT: &str = "count.txt";

fn master_alive() -> u8 {
    let file = File::open(COUNT).unwrap();
    let mut last_line = String::new();
    loop {
        let reader = BufReader::new(&file);
        if let Some(Ok(line)) = reader.lines().last() {
            if line != last_line {
                last_line = line;
            }
        }
    }

    return 8;
}

fn counter(){
    let time = Local::now().to_rfc3339();
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args[1]!="backup" {File::create("count.txt")?;}

    let count = master_alive();

    let mut file = OpenOptions::new().append(true).open("count.txt")?;
    file.write_all(b"\nAppending this line")?;
    file.write_all(b"\nAppending this other line")?;
    
    Ok(())
}