use clap::Parser;
use mbr::{parse_sector, PartitionTableNode};
use std::path::Path;

mod mbr;

#[derive(Debug, Parser)]
struct Arguments {
    image_path: String,
}

fn main() {
    let args = Arguments::parse();
    let mut root = PartitionTableNode::default();
    if let Err(e) = parse_sector(&mut root, &Path::new(&args.image_path), 0) {
        eprintln!("Error parsing MBR: {}", e);
    }
    if let Some(children) = root.children {
        println!("Partition Tables({}): {:#?}", children.len(), children);
    }
}
