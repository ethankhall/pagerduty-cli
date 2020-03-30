pub mod tree;
pub mod export;

use std::fs;

pub fn write_file(path: &str, contents: &str) -> std::io::Result<()> {
    if path == "-" {
        println!("{}", contents);
        Ok(())
    }  else {
        fs::write(path, contents)
    }
}