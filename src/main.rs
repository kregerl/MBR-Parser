use crate::mbr::parse_mbr;
use clap::Parser;
use gpt::parse_gpt;
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
        parse_gpt(path).unwrap();
    } else {
        display_mbr(root, show_chs);
    }
}
