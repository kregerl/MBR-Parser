use crate::{
    apm::{display_apm_partitions, parse_apm},
    mbr::parse_mbr,
};
use apm::is_apm_disk;
use clap::{Parser, Subcommand};
use gpt::{display_gpt, parse_gpt};
use mbr::display_mbr;
use mft::{display_mft, mft_to_csv, parse_pbr, timestomp_mft};
use std::path::{Path, PathBuf};

#[cfg(test)]
use std::io::Read;

mod apm;
mod bytestream;
mod gpt;
mod mbr;
mod mft;

#[derive(Debug, Parser)]
struct Arguments {
    image_path: String,
    #[arg(long)]
    show_chs: bool,
    #[arg(long)]
    extract_mft: bool,
    #[arg(long)]
    dump_mft: Option<String>,
    #[command(subcommand)]
    timestomp: Option<Timestomp>,
}

#[derive(Debug, Subcommand)]
pub enum Timestomp {
    /// Timestomp `file_name` with the `timestamp`
    Timestomp {
        /// Name of the file entry in the MFT
        file_name: String,
        /// Unix epoch timestamp to timestomp with
        timestamp: u64,
    },
}

fn main() {
    let args = Arguments::parse();
    let path = Path::new(&args.image_path);
    if is_apm_disk(&args.image_path).unwrap() {
        let partitions = parse_apm(&args.image_path).unwrap();
        display_apm_partitions(partitions);
    } else {
        // FIXME: This could all be done nicer if the signature is checked first.
        let mbr = parse_mbr(path);
        let mbr_node = match mbr {
            Ok(root_node) => root_node,
            Err(error) => panic!("Error parsing MBR: {}", error),
        };

        if mbr_node.is_gpt() {
            let partition_table = match parse_gpt(path) {
                Ok(partition_table) => partition_table,
                Err(error) => panic!("Error parsing GPT: {}", error),
            };

            if args.extract_mft || args.timestomp.is_some() || args.dump_mft.is_some() {
                let ntfs_partition = partition_table.into_iter().find(|entry| {
                    entry.get_partition_type_guid() == "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7"
                });
                let mft_records = match ntfs_partition {
                    Some(partition) => parse_pbr(path, partition.starting_lba()).unwrap(),
                    None => panic!("Could not find a `Microsoft basic data` partition."),
                };
                if args.dump_mft.is_some() {
                    mft_to_csv(mft_records, &args.dump_mft.unwrap()).unwrap();
                } else if args.extract_mft {
                    display_mft(mft_records);
                } else {
                    timestomp_mft(
                        &PathBuf::from(args.image_path),
                        mft_records,
                        args.timestomp.unwrap(),
                    );
                }
            } else {
                display_gpt(partition_table);
            }
        } else {
            if args.extract_mft || args.timestomp.is_some() || args.dump_mft.is_some() {
                let first_child = mbr_node.children.unwrap();
                let first_partition = first_child.get(0).unwrap();
                let mft_records = parse_pbr(path, first_partition.starting_lba() as u64).unwrap();

                if args.dump_mft.is_some() {
                    mft_to_csv(mft_records, &args.dump_mft.unwrap()).unwrap();
                } else if args.extract_mft {
                    display_mft(mft_records);
                } else {
                    timestomp_mft(
                        &PathBuf::from(args.image_path),
                        mft_records,
                        args.timestomp.unwrap(),
                    );
                }
            } else {
                display_mbr(mbr_node, args.show_chs);
            }
        }
    }
}

#[test]
pub fn test_open_drive() {
    use std::fs::OpenOptions;

    let path = Path::new("\\\\.\\PhysicalDrive0");
    let mut f = OpenOptions::new().read(true).open(path).unwrap();
    // Windows requires that physical drives are read in sectors.
    let mut x = vec![0u8; 512];
    f.read_exact(&mut x).unwrap();
    println!("Buffer: {:#?}", x);
}
