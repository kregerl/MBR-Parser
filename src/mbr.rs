use std::{
    fs::File,
    io::{self, Cursor, Read, Seek, SeekFrom},
    mem::{self, MaybeUninit},
    path::Path,
    slice,
};

use prettytable::{row, Row, Table};

const BOOTSTRAPER_LENGTH: usize = 446;
const SECTOR_SIZE: usize = 512;
const BOOT_SIGNATURE: [u8; 2] = [0x55, 0xAA];
const CHS_SECTOR_BIT_SIZE: u8 = 6;
const FIRST_TWO_BIT_MASK: u16 = 0b11000000;

#[derive(Debug)]
pub struct PartitionTableEntry {
    bootable: u8,
    starting_chs: [u8; 3],
    partition_type: u8,
    ending_chs: [u8; 3],
    lba_start: [u8; 4],
    num_sectors: [u8; 4],
}

impl PartitionTableEntry {
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
                partition_table_starting_lba + size,
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
                partition_table_starting_lba + size,
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

struct ByteStream {
    bytes: Cursor<Vec<u8>>,
    index: usize,
}

impl ByteStream {
    /// `Path` to file to read a sector(512 bytes starting at `starting_index`) starting from `image_offset_sectors`
    fn new(
        path: &Path,
        starting_index: Option<usize>,
        image_offest_sectors: u64,
    ) -> io::Result<Self> {
        Ok(Self {
            bytes: Cursor::new(Self::read_disk_image(path, image_offest_sectors)?),
            index: if let Some(index) = starting_index {
                index
            } else {
                0
            },
        })
    }

    /// Reads the first sector from an image (little-endian)
    fn read_disk_image(image_path: &Path, start_sector: u64) -> io::Result<Vec<u8>> {
        let mut image_file = File::open(image_path)?;
        let mut buffer = vec![0u8; SECTOR_SIZE];
        image_file.seek(SeekFrom::Start(start_sector * SECTOR_SIZE as u64))?;
        image_file.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    /// Get the next T from bytes without advancing
    fn peek<T>(&mut self) -> io::Result<T> {
        self.read_impl(false)
    }

    /// Advance and get the next T from bytes  
    fn read<T>(&mut self) -> io::Result<T> {
        self.read_impl(true)
    }

    /// Internal read to share code with `read` and `peek`
    fn read_impl<T>(&mut self, increment: bool) -> io::Result<T> {
        let num_bytes = mem::size_of::<T>();
        unsafe {
            // Allcoate memory for type T
            let mut s = MaybeUninit::<T>::uninit().assume_init();
            // Forms a writable slice from the pointer to the allocated struct and a size
            let buffer = slice::from_raw_parts_mut(&mut s as *mut T as *mut u8, num_bytes);
            // Offset bytes by `self.index`
            self.bytes.set_position(self.index as u64);
            // Read exactly enough bytes into `buffer`
            match self.bytes.read_exact(buffer) {
                Ok(()) => {
                    // If success, increment index and return filled struct
                    if increment {
                        self.index += num_bytes;
                    }
                    Ok(s)
                }
                Err(e) => {
                    // Deallocate the allocated memory on error
                    ::std::mem::forget(s);
                    Err(e)
                }
            }
        }
    }
}

/// Parse the entire mbr including extended partitions
pub fn parse_mbr(path: &Path, show_chs: bool) {
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
    parse_sector(&mut table, show_chs, path, true, 0, 0).unwrap();
    table.printstd();
}

/// first_ebr_lba :: The First extended block's Logical Block Address
fn parse_sector(
    table: &mut Table,
    show_chs: bool,
    path: &Path,
    is_first: bool,
    image_offset_sectors: u64,
    first_ebr_lba: u64,
) -> io::Result<()> {
    let mut stream = ByteStream::new(
        path,
        Some(BOOTSTRAPER_LENGTH as usize),
        image_offset_sectors,
    )?;

    // Assume we are always in the right place and stop the sector at BOOT_SIGNATURE
    while stream.peek::<[u8; 2]>()? != BOOT_SIGNATURE {
        let peek_byte = stream.peek::<u8>()?;
        // https://en.wikipedia.org/wiki/Master_boot_record#PTE:
        // MBRs only accept 0x80, 0x00 means inactive, and 0x01–0x7F stand for invalid
        if peek_byte != 0x00 && peek_byte != 0x80 && (0x01..0x7F).contains(&peek_byte) {
            break;
        }
        // Read table and discard zero'd out entries
        let partition_table_entry = stream.read::<PartitionTableEntry>()?;
        if partition_table_entry.is_empty() {
            break;
        }

        if partition_table_entry.is_extended_partition() {
            // If the partition is an extended partition, then we will jump to the EBR and parse the partition table there
            let start_lba = partition_table_entry.starting_lba() as u64;
            if is_first {
                table.add_row(partition_table_entry.table_row(image_offset_sectors, show_chs));
                // If this is the first extended partition table entry in the MBR, parse the next EBR at `start_lba` and set
                // `first_ebr_lba` to the start of the first EBR since all following EBR's starting LBA's are relative to the first EBR's LBA
                parse_sector(table, show_chs, path, false, start_lba, start_lba)?;
            } else {
                // If this is not the first extended partition table entry, parse the next EBR at `first_ebr_lba` + the `start_lba`
                // (relative to the first EBR's LBA) of this partition table entry. Leave the first EBR's LBA unchanged.
                parse_sector(
                    table,
                    show_chs,
                    path,
                    false,
                    first_ebr_lba + start_lba,
                    first_ebr_lba,
                )?;
            }
        } else {
            table.add_row(partition_table_entry.table_row(image_offset_sectors, show_chs));
        }
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
