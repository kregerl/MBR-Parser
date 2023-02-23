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
    let node = root;
    println!("Bootable    LBA Start(Starting CHS)    # of Sectors(Ending CHS)");
    println!("---------------------------------------------------------------");
    print_node(node, 1);
    
    // if let Some(children) = root.children {
    //     println!("Partition Tables({}): {:#?}", children.len(), children);
    // }
}

fn print_node(node: PartitionTableNode, mut depth: usize)  {
    if let Some(partition_table) = node.partition_table {
        //TODO: Print here
        println!("Partition Table: {}", partition_table);
    }
    if let Some(children) = node.children {
        depth += 1;
        for child in children {
            print_node(child, depth);
        }
    }
}
