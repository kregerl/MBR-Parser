use std::{
    io::{self},
    path::Path,
};

use prettytable::{row, Row, Table};

use crate::bytestream::ByteStream;

const BOOTSTRAPER_LENGTH: usize = 446;
// const BOOT_SIGNATURE: [u8; 2] = [0x55, 0xAA];
const CHS_SECTOR_BIT_SIZE: u8 = 6;
const FIRST_TWO_BIT_MASK: u16 = 0b11000000;
pub const GPT_PARTITION_TYPE: u8 = 0xee;

#[derive(Debug)]
pub struct MbrPartitionTableEntry {
    bootable: u8,
    starting_chs: [u8; 3],
    partition_type: u8,
    ending_chs: [u8; 3],
    lba_start: [u8; 4],
    num_sectors: [u8; 4],
}

impl MbrPartitionTableEntry {
    fn is_empty(&self) -> bool {
        self.bootable == 0
            && self.starting_chs.iter().all(|byte| *byte == 0)
            && self.partition_type == 0
            && self.ending_chs.iter().all(|byte| *byte == 0)
            && self.lba_start.iter().all(|byte| *byte == 0)
            && self.num_sectors.iter().all(|byte| *byte == 0)
    }

    fn is_extended_partition(&self) -> bool {
        self.partition_type == 0x05 || self.partition_type == 0x0F
    }

    fn starting_lba(&self) -> u32 {
        u32::from_le_bytes(self.lba_start)
    }

    fn num_sectors(&self) -> u32 {
        u32::from_le_bytes(self.num_sectors)
    }

    fn table_row(&self, image_offset_sectors: u64, show_chs: bool) -> Row {
        let partition_table_starting_lba = image_offset_sectors + self.starting_lba() as u64;
        let size = self.num_sectors() as u64;
        if show_chs {
            let starting_chs = self.parse_starting_chs();
            let ending_chs = self.parse_ending_chs();
            row![
                if self.bootable == 0x80 { "Yes" } else { "No" },
                partition_table_starting_lba,
                format!(
                    "({}, {}, {})",
                    starting_chs.0, starting_chs.1, starting_chs.2
                ),
                partition_table_starting_lba + size - 1,
                format!("({}, {}, {})", ending_chs.0, ending_chs.1, ending_chs.2),
                size,
                format!(
                    "{:#04x} :: {}",
                    self.partition_type,
                    lookup_partition_type(self.partition_type)
                ),
            ]
        } else {
            row![
                if self.bootable == 0x80 { "Yes" } else { "No" },
                partition_table_starting_lba,
                partition_table_starting_lba + size - 1,
                size,
                format!(
                    "{:#04x} :: {}",
                    self.partition_type,
                    lookup_partition_type(self.partition_type)
                ),
            ]
        }
    }

    fn parse_starting_chs(&self) -> (u16, u8, u8) {
        (
            Self::chs_cylinder(self.starting_chs),
            Self::chs_head(self.starting_chs),
            Self::chs_sector(self.starting_chs),
        )
    }

    fn parse_ending_chs(&self) -> (u16, u8, u8) {
        (
            Self::chs_cylinder(self.ending_chs),
            Self::chs_head(self.ending_chs),
            Self::chs_sector(self.ending_chs),
        )
    }

    fn chs_head(chs: [u8; 3]) -> u8 {
        chs[0]
    }

    fn chs_sector(chs: [u8; 3]) -> u8 {
        chs[1] & ((1 << CHS_SECTOR_BIT_SIZE) - 1)
    }

    fn chs_cylinder(chs: [u8; 3]) -> u16 {
        ((chs[1] as u16 & FIRST_TWO_BIT_MASK) << 2) | (chs[2] as u16)
    }
}

#[derive(Debug, Default)]
pub struct MbrPartitionTableEntryNode {
    partition_table_entry: Option<MbrPartitionTableEntry>,
    pub children: Option<Vec<MbrPartitionTableEntryNode>>,
    image_offset_sectors: u64,
}

impl MbrPartitionTableEntryNode {
    fn new(partition_table_entry: MbrPartitionTableEntry, image_offset_sectors: u64) -> Self {
        Self {
            partition_table_entry: Some(partition_table_entry),
            children: None,
            image_offset_sectors,
        }
    }

    fn add_child(&mut self, node: MbrPartitionTableEntryNode) {
        match &mut self.children {
            Some(children) => children.push(node),
            None => self.children = Some(vec![node]),
        }
    }

    fn table_row(&self, show_chs: bool) -> Row {
        if let Some(entry) = &self.partition_table_entry {
            entry.table_row(self.image_offset_sectors, show_chs)
        } else {
            row![]
        }
    }

    fn is_extended_partition(&self) -> bool {
        if let Some(entry) = &self.partition_table_entry {
            entry.is_extended_partition()
        } else {
            false
        }
    }

    pub fn is_gpt(&self) -> bool {
        if let Some(children) = &self.children {
            let gpt_partition = children
                .iter()
                .find(|child| {
                    if child.partition_table_entry.is_none() {
                        false
                    } else {
                        child.partition_table_entry.as_ref().unwrap().partition_type == GPT_PARTITION_TYPE
                    }
                });
            match gpt_partition {
                Some(_) => true,
                None => false,
            }
        } else {
            false
        }
    }
}

fn print_nodes(
    table: &mut Table,
    node: MbrPartitionTableEntryNode,
    show_chs: bool,
    is_first: bool,
) {
    if let Some(children) = node.children {
        for child_node in children {
            if child_node.is_extended_partition() && is_first {
                table.add_row(child_node.table_row(show_chs));
            }
            print_nodes(table, child_node, show_chs, false);
        }
    } else {
        table.add_row(node.table_row(show_chs));
    }
}

fn parse_sector(
    node: &mut MbrPartitionTableEntryNode,
    path: &Path,
    is_first: bool,
    image_offset_sector: u64,
    first_ebr_lba: u64,
) -> io::Result<()> {
    let mut stream = ByteStream::new(path, Some(BOOTSTRAPER_LENGTH as usize), image_offset_sector)?;

    // Boot record can only have at max 4 entries.
    for _ in 0..4 {
        let peek_byte = stream.peek::<u8>()?;
        // https://en.wikipedia.org/wiki/Master_boot_record#PTE:
        // MBRs only accept 0x80, 0x00 means inactive, and 0x01–0x7F stand for invalid
        if peek_byte != 0x00 && peek_byte != 0x80 && (0x01..0x7F).contains(&peek_byte) {
            break;
        }
        // Read table and stop at zero'd out entries
        let partition_table_entry = stream.read::<MbrPartitionTableEntry>()?;
        if partition_table_entry.is_empty() {
            break;
        }

        let next_node = if partition_table_entry.is_extended_partition() {
            // If the partition is an extended partition, then we will jump to the EBR and parse the partition table there
            let start_lba = partition_table_entry.starting_lba() as u64;
            let mut next_node =
                MbrPartitionTableEntryNode::new(partition_table_entry, image_offset_sector);
            if is_first {
                // table.add_row(partition_table_entry.table_row(image_offset_sectors, show_chs));
                // If this is the first extended partition table entry in the MBR, parse the next EBR at `start_lba` and set
                // `first_ebr_lba` to the start of the first EBR since all following EBR's starting LBA's are relative to the first EBR's LBA
                parse_sector(&mut next_node, path, false, start_lba, start_lba)?;
            } else {
                // If this is not the first extended partition table entry, parse the next EBR at `first_ebr_lba` + the `start_lba`
                // (relative to the first EBR's LBA) of this partition table entry. Leave the first EBR's LBA unchanged.
                parse_sector(
                    &mut next_node,
                    path,
                    false,
                    first_ebr_lba + start_lba,
                    first_ebr_lba,
                )?;
            }
            next_node
        } else {
            MbrPartitionTableEntryNode::new(partition_table_entry, image_offset_sector)
        };
        node.add_child(next_node);
    }

    Ok(())
}

fn lookup_partition_type(partition_type: u8) -> String {
    match partition_type {
        0x0 => "Empty",
        0x1 => "FAT12",
        0x2 => "XENIX root",
        0x3 => "XENIX usr",
        0x4 => "FAT16 <32M",
        0x5 => "Extended",
        0x6 => "FAT16",
        0x7 => "HPFS/NTFS/exFAT",
        0x8 => "AIX",
        0x9 => "AIX bootable",
        0xa => "OS/2 Boot Manag",
        0xb => "W95 FAT32",
        0xc => "W95 FAT32 (LBA)",
        0xe => "W95 FAT16 (LBA)",
        0xf => "W95 Ext'd (LBA)",
        0x10 => "OPUS",
        0x11 => "Hidden FAT12",
        0x12 => "Compaq diagnost",
        0x14 => "Hidden FAT16 <3",
        0x16 => "Hidden FAT16",
        0x17 => "Hidden HPFS/NTF",
        0x18 => "AST SmartSleep",
        0x1b => "Hidden W95 FAT3",
        0x1c => "Hidden W95 FAT3",
        0x1e => "Hidden W95 FAT1",
        0x24 => "NEC DOS",
        0x27 => "Hidden NTFS Win",
        0x39 => "Plan 9",
        0x3c => "PartitionMagic",
        0x40 => "Venix 80286",
        0x41 => "PPC PReP Boot",
        0x42 => "SFS",
        0x4d => "QNX4.x",
        0x4e => "QNX4.x 2nd part",
        0x4f => "QNX4.x 3rd part",
        0x50 => "OnTrack DM",
        0x51 => "OnTrack DM6 Aux",
        0x52 => "CP/M",
        0x53 => "OnTrack DM6 Aux",
        0x54 => "OnTrackDM6",
        0x55 => "EZ-Drive",
        0x56 => "Golden Bow",
        0x5c => "Priam Edisk",
        0x61 => "SpeedStor",
        0x63 => "GNU HURD or Sys",
        0x64 => "Novell Netware",
        0x65 => "Novell Netware",
        0x70 => "DiskSecure Mult",
        0x75 => "PC/IX",
        0x80 => "Old Minix",
        0x81 => "Minix / old Lin",
        0x82 => "Linux swap / So",
        0x83 => "Linux",
        0x84 => "OS/2 hidden or",
        0x85 => "Linux extended",
        0x86 => "NTFS volume set",
        0x87 => "NTFS volume set",
        0x88 => "Linux plaintext",
        0x8e => "Linux LVM",
        0x93 => "Amoeba",
        0x94 => "Amoeba BBT",
        0x9f => "BSD/OS",
        0xa0 => "IBM Thinkpad hi",
        0xa5 => "FreeBSD",
        0xa6 => "OpenBSD",
        0xa7 => "NeXTSTEP",
        0xa8 => "Darwin UFS",
        0xa9 => "NetBSD",
        0xab => "Darwin boot",
        0xaf => "HFS / HFS+",
        0xb7 => "BSDI fs",
        0xb8 => "BSDI swap",
        0xbb => "Boot Wizard hid",
        0xbc => "Acronis FAT32 L",
        0xbe => "Solaris boot",
        0xbf => "Solaris",
        0xc1 => "DRDOS/sec (FAT-",
        0xc4 => "DRDOS/sec (FAT-",
        0xc6 => "DRDOS/sec (FAT-",
        0xc7 => "Syrinx",
        0xda => "Non-FS data",
        0xdb => "CP/M / CTOS / .",
        0xde => "Dell Utility",
        0xdf => "BootIt",
        0xe1 => "DOS access",
        0xe3 => "DOS R/O",
        0xe4 => "SpeedStor",
        0xea => "Rufus alignment",
        0xeb => "BeOS fs",
        0xee => "GPT",
        0xef => "EFI (FAT-12/16/",
        0xf0 => "Linux/PA-RISC b",
        0xf1 => "SpeedStor",
        0xf4 => "SpeedStor",
        0xf2 => "DOS secondary",
        0xfb => "VMware VMFS",
        0xfc => "VMware VMKCORE",
        0xfd => "Linux raid auto",
        0xfe => "LANstep",
        0xff => "BBT",
        _ => "Unknown Partition Type",
    }
    .into()
}

pub fn parse_mbr(path: &Path) -> io::Result<MbrPartitionTableEntryNode> {
    let mut root = MbrPartitionTableEntryNode::default();
    parse_sector(&mut root, path, true, 0, 0)?;
    Ok(root)
}

pub fn display_mbr(root: MbrPartitionTableEntryNode, show_chs: bool) {
    let mut table = Table::new();
    let row = if show_chs {
        row![
            "Bootable",
            "LBA Starting Sector",
            "Starting CHS",
            "LBA Ending Sector",
            "Ending CHS",
            "Total Sectors",
            "Partition Type"
        ]
    } else {
        row![
            "Bootable",
            "LBA Starting Sector",
            "LBA Ending Sector",
            "Total Sectors",
            "Partition Type"
        ]
    };
    table.add_row(row);
    print_nodes(&mut table, root, show_chs, true);
    table.printstd();
}
