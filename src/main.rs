use mbr::{parse_sector, PartitionTableNode};
use std::path::Path;

mod mbr;

fn main() {
    let mut root = PartitionTableNode::default();
    if let Err(e) = parse_sector(&mut root, &Path::new("./mbr_test.dd"), 0) {
        eprintln!("Error parsing MBR: {}", e);
    }
    if let Some(children) = root.children {
        println!("Partition Tables({}): {:#?}", children.len(), children);
    }
}
