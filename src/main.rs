use crate::mbr::parse_mbr;
use clap::Parser;
use gpt::{display_gpt, parse_gpt};
use mbr::{display_mbr};
use mft::parse_pbr;
use std::{path::Path, fs::File, io::Read};

mod bytestream;
mod gpt;
mod mbr;
mod mft;

#[derive(Debug, Parser)]
struct Arguments {
    image_path: String,
    #[arg(long)]
    show_chs: bool,
    #[arg(long)]
    extract_mft: bool,
}

fn main() {
    let args = Arguments::parse();
    let show_chs = args.show_chs;
    let path = Path::new(&args.image_path);
    let mbr = parse_mbr(path);
    match mbr {
        Ok(root) => {
            if root.is_gpt() {
                let partition_table = parse_gpt(path);
                match partition_table {
                    Ok(partition_table_entries) => {
                        display_gpt(partition_table_entries);
                    }
                    Err(e) => eprintln!("Error parsing GPT: {}", e),
                }
            } else {
                if args.extract_mft {
                    let first_child = root.children.unwrap();
                    let first_partition = first_child.get(0).unwrap();
                    parse_pbr(path, first_partition).unwrap();
                } else {
                    display_mbr(root, show_chs);
                }
            }
        }
        Err(e) => eprintln!("Parse Error: {}", e),
    }
}

#[test]
pub fn test_open_drive() {
    use std::fs::{OpenOptions};

    let path = Path::new("\\\\.\\PhysicalDrive0");
    let mut f = OpenOptions::new().read(true).open(path).unwrap();
    // Windows requires that physical drives are read in sectors.
    let mut x = vec![0u8; 512];
    f.read_exact(&mut x).unwrap();
    println!("Buffer: {:#?}", x);
}