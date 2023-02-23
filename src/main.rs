use std::path::Path;
use mbr::parse_mbr;

mod mbr;

fn main() {
    if let Err(e) = parse_mbr(&Path::new("./mbr_test.dd")) {
        eprintln!("Error parsing MBR: {}", e);
    }
}