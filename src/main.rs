use crate::mbr::parse_mbr;
use clap::Parser;
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
    parse_mbr(&Path::new(&args.image_path), args.show_chs);
}
