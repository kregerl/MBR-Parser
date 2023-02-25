use crate::mbr::parse_mbr;
use clap::Parser;
use gpt::{parse_gpt, display_gpt};
use mbr::display_mbr;
use std::path::Path;

mod bytestream;
mod gpt;
mod mbr;

#[derive(Debug, Parser)]
struct Arguments {
    image_path: String,
    #[arg(long)]
    show_chs: bool,
}

fn main() {
    let args = Arguments::parse();
    let show_chs = args.show_chs;
    let path = Path::new(&args.image_path);
    let root = parse_mbr(path);
    if root.is_gpt() {
        let partition_table = parse_gpt(path);
        match partition_table {
            Ok(partition_table_entries) => {
                display_gpt(partition_table_entries);
            },
            Err(e) => eprintln!("Error parsing GPT: {}", e),
        }
    } else {
        display_mbr(root, show_chs);
    }
}
