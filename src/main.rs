use clap::Parser;
use mbr::{parse_sector, PartitionTableNode};
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
    let mut root = PartitionTableNode::default();
    if let Err(e) = parse_sector(&mut root, &Path::new(&args.image_path), 0) {
        eprintln!("Error parsing MBR: {}", e);
    }
    let node = root;
    println!("| {:<10} | {:<12} | {:<22} | {:<12} | {:<22} | {:<12} |", "Bootable", "LBA Start", "Starting CHS", "LBA End", "Ending CHS", "# Sectors");
    println!("{}", str::repeat("-", 109));
    print_node(node);
}

fn print_node(node: PartitionTableNode)  {
    if let Some(partition_table) = node.partition_table {
        println!("{} -- {}", partition_table, partition_table.partition_type);
    }
    if let Some(children) = node.children {
        for child in children {
            print_node(child);
        }
    } else {
    }
}
