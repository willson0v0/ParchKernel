use std::io::{Result, Write};
use std::fs::{File, OpenOptions, read_dir};
use chrono::{DateTime, Utc};

fn updata_version_number() -> Result<()> {
    let now: DateTime<Utc> = Utc::now();
    let mut fo = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("src/version.rs")
        .unwrap();
    
    let ni = format!(r#"//! This is a uname constant, and will be update automatically on building.
/// NOTE: This will be found and modified by build.rs. ***DONT CHANGE THIS LINE MANUALLY!!!!***
pub const VERSION : &str = "{}";"#, now.to_rfc2822());
    writeln!(fo, "{}", ni)?;
    Ok(())
}

fn main() {
    println!("cargo:rerun-if-changed=./src/");
	updata_version_number().unwrap();
}