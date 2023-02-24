use clap::Parser;
use mbr::{parse_sector};
use std::path::Path;

mod mbr;

#[derive(Debug, Parser)]
struct Arguments {
    image_path: String,
    #[arg(long)]
    show_chs: bool,
}

fn main() {
    let args = Arguments::parse();
    println!("| {:<4} | {:<4} | {:<12} | {:<12} | {:<12} |", "PT", "BOOT", "START", "END", "SIZE");
    println!("-------------------------------------------------------------");
    parse_sector(&Path::new(&args.image_path), true, 0, 0).unwrap();
}
