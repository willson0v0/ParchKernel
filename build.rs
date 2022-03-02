use std::io::{Result, Write};
use std::fs::{OpenOptions};
use chrono::{DateTime, Utc};
use csv::Reader;

fn update_version_number() -> Result<()> {
    let now: DateTime<Utc> = Utc::now();
    let mut fo = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("src/version.rs")?;
    writeln!(fo, "//! This is a uname constant, and will be update automatically on building.")?;
    writeln!(fo, "/// NOTE: This will be modified by build.rs on build. ***DONT CHANGE THESE LINE MANUALLY!!!!***")?;
    writeln!(fo, r#"pub const VERSION : &str = "{}";"#, now.to_rfc2822())?;
    writeln!(fo, "/// NOTE: This will be modified by build.rs on build. ***DONT CHANGE THESE LINE MANUALLY!!!!***")?;
    writeln!(fo, "pub const COMPILE_EPOCH : usize = {};", now.timestamp())?;
    Ok(())
}

fn update_syscall_number() -> Result<()> {
    let fi = OpenOptions::new()
        .read(true)
        .open("../syscall_num.csv")?;
    let mut fo = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("src/syscall/syscall_num.rs")?;
    let mut rdr = Reader::from_reader(fi);
    for result in rdr.records() {
        let record = result?;
        println!("{} => {}", record.get(0).unwrap(), record.get(1).unwrap());
        writeln!(fo, "pub const SYSCALL_{:<10}: usize = {:>3};", record.get(0).unwrap().to_ascii_uppercase(), record.get(1).unwrap())?;
    }
    Ok(())
}

fn main() {
    println!("cargo:rerun-if-changed=./src/");
    println!("cargo:rerun-if-changed=../syscall_num.csv");
	update_version_number().unwrap();
    update_syscall_number().unwrap();
}