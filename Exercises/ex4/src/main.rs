use std::fs::{File,OpenOptions};
use std::io::prelude::*;
use std::process::Command;
use std::env;
use std::io::{BufRead, BufReader};
use std::time::{Duration,Instant};
use std::thread::sleep;

fn master_alive() -> u8 {
    let file = File::open("count.txt").unwrap();
    let mut last_line = String::new();
    let mut update_time = Instant::now();

    while update_time.elapsed() <= Duration::from_millis(1500) {
        let reader = BufReader::new(&file);
        if let Some(Ok(line)) = reader.lines().last() {
            if line != last_line {
                last_line = line;
                update_time = Instant::now();
            }
        }
        sleep(Duration::from_millis(100));
    }

    return last_line.parse().unwrap();
}

fn counter(mut count: u8){
    let mut file = OpenOptions::new().append(true).open("count.txt").unwrap();
    loop {
        count += 1;
        writeln!(file, "{}", count).unwrap();
        println!("{}",count);
        sleep(Duration::from_millis(1000));
    }
}

fn main() -> std::io::Result<()> {

    // Initialize file if process is not a backup
    let args: Vec<String> = env::args().collect();
    if (args.len() == 1) || (args[1] != "backup") {
        let mut file = File::create("count.txt")?;
        writeln!(file, "{}", 0).unwrap();
    }

    // Poll master until it is dead, return the state
    let count = master_alive();

    // Spawn a backup
    Command::new("cmd")
    .args(&["/C", "start", "cmd", "/K", "target\\debug\\ex4.exe", "backup"])
    .spawn()
    .unwrap();

    // Continue counting from the previous state
    counter(count);

    Ok(())
}