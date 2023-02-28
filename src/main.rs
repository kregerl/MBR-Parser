use crate::mbr::parse_mbr;
use clap::Parser;
use gpt::{display_gpt, parse_gpt};
use mbr::{display_mbr, parse_pbr};
use std::path::Path;

mod bytestream;
mod gpt;
mod mbr;

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
