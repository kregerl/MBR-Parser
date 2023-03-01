use crate::bytestream::{interpret_bytes_as_utf16, ByteStream, Readable, SECTOR_SIZE};
use prettytable::{row, Row, Table};
use std::{
    fmt::Display,
    io::{self, Seek},
    path::Path,
    string::FromUtf8Error,
};

const BOOTSTRAPER_LENGTH: usize = 446;
const BOOT_SIGNATURE: [u8; 2] = [0x55, 0xAA];
const CHS_SECTOR_BIT_SIZE: u8 = 6;
const FIRST_TWO_BIT_MASK: u16 = 0b11000000;
pub const GPT_PARTITION_TYPE: u8 = 0xee;

#[derive(Debug)]
pub struct MbrPartitionTableEntry {
    bootable: u8,
    starting_chs: [u8; 3],
    partition_type: u8,
    ending_chs: [u8; 3],
    lba_start: u32,
    num_sectors: u32,
}

impl Readable for MbrPartitionTableEntry {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            bootable: reader.read()?,
            starting_chs: reader.read_byte_array::<3>()?,
            partition_type: reader.read()?,
            ending_chs: reader.read_byte_array::<3>()?,
            lba_start: reader.read()?,
            num_sectors: reader.read()?,
        })
    }
}

impl MbrPartitionTableEntry {
    fn is_empty(&self) -> bool {
        self.bootable == 0
            && self.starting_chs.iter().all(|byte| *byte == 0)
            && self.partition_type == 0
            && self.ending_chs.iter().all(|byte| *byte == 0)
            && self.lba_start == 0
            && self.num_sectors == 0
    }

    fn is_extended_partition(&self) -> bool {
        self.partition_type == 0x05 || self.partition_type == 0x0F
    }

    fn starting_lba(&self) -> u32 {
        self.lba_start
    }

    fn num_sectors(&self) -> u32 {
        self.num_sectors
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
            let gpt_partition = children.iter().find(|child| {
                if child.partition_table_entry.is_none() {
                    false
                } else {
                    child.partition_table_entry.as_ref().unwrap().partition_type
                        == GPT_PARTITION_TYPE
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

    pub fn starting_lba(&self) -> u32 {
        if let Some(entry) = &self.partition_table_entry {
            entry.starting_lba()
        } else {
            0
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

// FIXME: Pass ByteStream as parmeter instead of path
fn parse_sector(
    node: &mut MbrPartitionTableEntryNode,
    path: &Path,
    is_first: bool,
    image_offset_sector: u64,
    first_ebr_lba: u64,
) -> io::Result<()> {
    // , Some(BOOTSTRAPER_LENGTH as usize), image_offset_sector
    let mut stream = ByteStream::new(path)?;
    let _ = stream
        .jump_to_byte((image_offset_sector * SECTOR_SIZE as u64) + BOOTSTRAPER_LENGTH as u64)?;

    // Boot record can only have at max 4 entries.
    for _ in 0..4 {
        // Read table and stop at zero'd out entries
        let partition_table_entry = stream.read::<MbrPartitionTableEntry>()?;
        if partition_table_entry.is_empty() {
            break;
        }

        // https://en.wikipedia.org/wiki/Master_boot_record#PTE:
        // MBRs only accept 0x80, 0x00 means inactive, and 0x01–0x7F stand for invalid
        let bootable = partition_table_entry.bootable;
        if bootable != 0x00 && bootable != 0x80 && (0x01..0x7F).contains(&bootable) {
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

#[derive(Debug)]
struct NtfsPartitionBootRecord {
    jump_instruction: [u8; 3],
    oem_id: [u8; 8],
    // BPB
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    device_type: u8,
    number_of_sectors_in_volume: u64,
    mft_lcn: u64, // Logical cluster number where the MFT starts.
    backup_mft_lcn: u64,
    // - If this value, when read in two’s complement, is positive,
    //   i.e. if its value goes from 00h to 7Fh (0000 0000 a 0111 1111),
    //   it actually designates the number of clusters per register
    // - If this value, when read in two’s complement, is negative,
    //   i.e. if its value goes from 80h to FFh (1000 0000 a 1111 1111), the
    //   size in bytes of each register will be equal to  2 to the power of the byte absolute value.
    mft_size: i8,
    number_of_clusters_per_index_buffer: u8,
    serial_number: [u8; 8],
    error_bytes: Vec<u8>,
}

impl Readable for NtfsPartitionBootRecord {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        // 13 Bytes of error marking space (0x0e - 0x14, 0x16 - 0x17, 0x20 - 0x23)
        let mut error_bytes = Vec::with_capacity(13);
        // 22 Bytes of unused space (0x18 - 0x1F, 0x24 - 0x27, 0x41 - 0x43, 0x45 - 0x47, 0x50 - 0x53)
        let mut unused_space = Vec::with_capacity(22);

        let jump_instruction = reader.read_byte_array::<3>()?;
        // Interpreted as a string
        let oem_id = reader.read_byte_array::<8>()?;
        let bytes_per_sector = reader.read::<u16>()?;
        let sectors_per_cluster = reader.read::<u8>()?;
        error_bytes.append(&mut reader.read_byte_array::<7>()?.to_vec());
        let device_type = reader.read::<u8>()?;
        error_bytes.append(&mut reader.read_byte_array::<2>()?.to_vec());
        unused_space.append(&mut reader.read_byte_array::<8>()?.to_vec());
        error_bytes.append(&mut reader.read_byte_array::<4>()?.to_vec());
        unused_space.append(&mut reader.read_byte_array::<4>()?.to_vec());
        let number_of_sectors_in_volume = reader.read::<u64>()?;
        let mft_lcn = reader.read::<u64>()?;
        let backup_mft_lcn = reader.read::<u64>()?;
        let mft_size = reader.read::<i8>()?;
        unused_space.append(&mut reader.read_byte_array::<3>()?.to_vec());
        let number_of_clusters_per_index_buffer = reader.read::<u8>()?;
        unused_space.append(&mut reader.read_byte_array::<3>()?.to_vec());
        let serial_number = reader.read_byte_array::<8>()?;
        unused_space.append(&mut reader.read_byte_array::<4>()?.to_vec());

        Ok(Self {
            jump_instruction,
            oem_id,
            bytes_per_sector,
            sectors_per_cluster,
            device_type,
            number_of_sectors_in_volume,
            mft_lcn,
            backup_mft_lcn,
            mft_size,
            number_of_clusters_per_index_buffer,
            serial_number,
            error_bytes,
        })
    }
}

impl NtfsPartitionBootRecord {
    pub fn oem_id_str(&self) -> Result<String, FromUtf8Error> {
        String::from_utf8(self.oem_id.to_vec()).and_then(|s| Ok(s.trim().into()))
    }
}

pub fn parse_pbr(
    path: &Path,
    partition_table_entry: &MbrPartitionTableEntryNode,
) -> io::Result<()> {
    let starting_lba = partition_table_entry.starting_lba() as u64;

    let mut stream = ByteStream::new(path)?;
    stream.jump_to_sector(starting_lba)?;
    let partition_boot_record = stream.read::<NtfsPartitionBootRecord>()?;
    match partition_boot_record.oem_id_str().as_deref() {
        Ok("NTFS") => {
            // 510(Sector size - signature) - 84 (PBR btyes read) = 426 Boot code
            let _ = stream.read_byte_array::<426>()?;
            assert_eq!(
                stream.read_byte_array::<2>()?,
                BOOT_SIGNATURE,
                "End of sector was not reached"
            );

            println!("PBR: {:#?}", partition_boot_record);
            let mft_lba = starting_lba
                + (partition_boot_record.mft_lcn
                    * partition_boot_record.sectors_per_cluster as u64);
            // let backup_mft_lba = starting_lba + (partition_boot_record.backup_mft_lcn * partition_boot_record.sectors_per_cluster as u64);
            parse_mft(&mut stream, mft_lba)?;
        }
        Err(e) => eprintln!("Error parsing OEM ID: {}", e),
        _ => eprintln!("Cannot parse $MFT of a non-NTFS partition"),
    }
    // let mft_lba = starting_lba + u64::from_le_bytes(pbr.mft_lcn) * pbr.sectors_per_cluster as u64;
    // let backup_mft_lba = starting_lba + u64::from_le_bytes(pbr.backup_mft_lcn) * pbr.sectors_per_cluster as u64;
    // println!("mft_lba: {:02x}", mft_lba * 512);
    // println!("mft_size: {}", pbr.mft_size);

    // parse_file_entry(path, mft_lba as usize)?;

    Ok(())
}

#[derive(Debug)]
struct MftFileDescriptor {
    signature: [u8; 4],
    offest_of_update_seq: u16,
    size_of_update_seq: u16,
    log_file_seq_nr: u64,
    use_count: u8,
    deletion_count: u8,
    hard_link_count: u16,
    offset_fist_attribute: u16,
    // 0x00 == Register free, 0x01 == Register in use, 0x02 == Register is a directory
    flags: u16,
    file_size_on_disk: u32,
    space_allocated: u32,
    base_register: u64,
    next_attribute_id: u16,
    update_sequence_number: u16,
    update_sequence: u32,
}

impl Readable for MftFileDescriptor {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            signature: reader.read_byte_array::<4>()?,
            offest_of_update_seq: reader.read::<u16>()?,
            size_of_update_seq: reader.read::<u16>()?,
            log_file_seq_nr: reader.read::<u64>()?,
            use_count: reader.read::<u8>()?,
            deletion_count: reader.read::<u8>()?,
            hard_link_count: reader.read::<u16>()?,
            offset_fist_attribute: reader.read::<u16>()?,
            flags: reader.read::<u16>()?,
            file_size_on_disk: reader.read::<u32>()?,
            space_allocated: reader.read::<u32>()?,
            base_register: reader.read::<u64>()?,
            next_attribute_id: reader.read::<u16>()?,
            update_sequence_number: reader.read::<u16>()?,
            update_sequence: reader.read::<u32>()?,
        })
    }
}

/// Struct for holding the common values between all attribute headers.
/// Flags
/// 0x0001 == Compressed
/// 0x4000 == Encrypted
/// 0x8000 == Sparse
#[derive(Debug)]
struct CommonAttributeHeader {
    attribute_type: u32,
    length: u32,
    non_resident_flag: u8,
    name_length: u8,
    name_offset: u16,
    flags: [u8; 2],
    attribute_id: u16,
}

impl Readable for CommonAttributeHeader {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            attribute_type: reader.read::<u32>()?,
            length: reader.read::<u32>()?,
            non_resident_flag: reader.read::<u8>()?,
            name_length: reader.read::<u8>()?,
            name_offset: reader.read::<u16>()?,
            flags: reader.read_byte_array::<2>()?,
            attribute_id: reader.read::<u16>()?,
        })
    }
}

/// Data relating to resident attributes only.
#[derive(Debug)]
struct ResidentAttributeHeader {
    attribute_length: u32,
    attribute_offset: u16,
    indexed_flag: u8,
}

impl Readable for ResidentAttributeHeader {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            attribute_length: reader.read::<u32>()?,
            attribute_offset: reader.read::<u16>()?,
            indexed_flag: reader.read::<u8>()?,
        })
    }
}

/// Data specific to non resident attribute headers
#[derive(Debug)]
struct NonResidentAttributeHeader {
    starting_vcn: u64,
    ending_vcn: u64,
    data_runs_offset: u16,
    compression_unit_size: u16,
    allocated_size_of_attribute: u64,
    real_size_of_attribute: u64,
    initialized_data_size: u64,
}

impl Readable for NonResidentAttributeHeader {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        let starting_vcn = reader.read::<u64>()?;
        let ending_vcn = reader.read::<u64>()?;
        let data_runs_offset = reader.read::<u16>()?;
        let compression_unit_size = reader.read::<u16>()?;
        // Discard 4 bytes of 0 padding
        let _ = reader.read_byte_array::<4>()?;
        let allocated_size_of_attribute = reader.read::<u64>()?;
        let real_size_of_attribute = reader.read::<u64>()?;
        let initialized_data_size = reader.read::<u64>()?;
        Ok(Self {
            starting_vcn,
            ending_vcn,
            data_runs_offset,
            compression_unit_size,
            allocated_size_of_attribute,
            real_size_of_attribute,
            initialized_data_size,
        })
    }
}

// The MFT Must have one of these attribute headers.
#[derive(Debug)]
enum AttributeHeader {
    ResidentNoName {
        common_header: CommonAttributeHeader,
        resident_header: ResidentAttributeHeader,
    },
    ResidentNamed {
        common_header: CommonAttributeHeader,
        resident_header: ResidentAttributeHeader,
        attribute_name: String,
    },
    NonResidentNoName {
        common_header: CommonAttributeHeader,
        non_resident_header: NonResidentAttributeHeader,
    },
    NonResidentNamed {
        common_header: CommonAttributeHeader,
        non_resident_header: NonResidentAttributeHeader,
        attribute_name: String,
    },
}

impl Readable for AttributeHeader {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        let common_attribute_header = reader.read::<CommonAttributeHeader>()?;
        // If the non resident flag is off
        Ok(if common_attribute_header.non_resident_flag == 0 {
            let resident_attribute_header = reader.read::<ResidentAttributeHeader>()?;
            // Discard one byte for padding for resident attributes
            let _ = reader.read::<u8>()?;

            // if attributes has a name
            if common_attribute_header.name_length > 0 {
                // Attribute name is 2*N since it is stored in unicode(2 bytes)
                let attribute_name_bytes =
                    reader.read_raw(common_attribute_header.name_length as usize * 2)?;
                let name = interpret_bytes_as_utf16(&attribute_name_bytes)
                    .expect("Invalid utf16 bytes in attribute header.");
                AttributeHeader::ResidentNamed {
                    common_header: common_attribute_header,
                    resident_header: resident_attribute_header,
                    attribute_name: name,
                }
            } else {
                AttributeHeader::ResidentNoName {
                    common_header: common_attribute_header,
                    resident_header: resident_attribute_header,
                }
            }
        } else {
            // Read the non resident attribute header
            let non_resident_attribute_header = reader.read::<NonResidentAttributeHeader>()?;
            if common_attribute_header.name_length > 0 {
                // If is is named, read the name
                let attribute_name_bytes =
                    reader.read_raw(common_attribute_header.name_length as usize * 2)?;
                let name = interpret_bytes_as_utf16(&attribute_name_bytes)
                    .expect("Invalid utf16 bytes in attribute header.");
                AttributeHeader::NonResidentNamed {
                    common_header: common_attribute_header,
                    non_resident_header: non_resident_attribute_header,
                    attribute_name: name,
                }
            } else {
                // Otherwise return the nameless non resident header
                AttributeHeader::NonResidentNoName {
                    common_header: common_attribute_header,
                    non_resident_header: non_resident_attribute_header,
                }
            }
        })
    }
}

#[derive(Debug, Clone)]
struct PermissionParseError;

impl Display for PermissionParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid NTFS permissions integer")
    }
}

#[derive(Debug)]
#[repr(u32)]
enum NtfsPermissions {
    ReadOnly = 0x0001,
    Hidden = 0x0002,
    System = 0x0004,
    Archive = 0x0020,
    Device = 0x0040,
    Normal = 0x0080,
    Temporary = 0x0100,
    SparseFile = 0x0200,
    ReparseFile = 0x0400,
    Compressed = 0x0800,
    Offline = 0x1000,
    NotContentIndexed = 0x2000,
    Encrypted = 0x4000,
}

impl TryFrom<u32> for NtfsPermissions {
    type Error = PermissionParseError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0x0001 => Ok(NtfsPermissions::ReadOnly),
            0x0002 => Ok(NtfsPermissions::Hidden),
            0x0004 => Ok(NtfsPermissions::System),
            0x0020 => Ok(NtfsPermissions::Archive),
            0x0040 => Ok(NtfsPermissions::Device),
            0x0080 => Ok(NtfsPermissions::Normal),
            0x0100 => Ok(NtfsPermissions::Temporary),
            0x0200 => Ok(NtfsPermissions::SparseFile),
            0x0400 => Ok(NtfsPermissions::ReparseFile),
            0x0800 => Ok(NtfsPermissions::Compressed),
            0x1000 => Ok(NtfsPermissions::Offline),
            0x2000 => Ok(NtfsPermissions::NotContentIndexed),
            0x4000 => Ok(NtfsPermissions::Encrypted),
            _ => Err(PermissionParseError),
        }
    }
}

#[derive(Debug)]
struct StandardInformation {
    datetime_file_creation: u64,
    datetime_file_modification: u64,
    datetime_mft_modification: u64,
    datetime_file_reading: u64,
    file_permission_flags: u32,
    maximum_number_versions: u32,
    version_number: u64,
}

impl Readable for StandardInformation {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            datetime_file_creation: reader.read::<u64>()?,
            datetime_file_modification: reader.read::<u64>()?,
            datetime_mft_modification: reader.read::<u64>()?,
            datetime_file_reading: reader.read::<u64>()?,
            file_permission_flags: reader.read::<u32>()?,
            maximum_number_versions: reader.read::<u32>()?,
            version_number: reader.read::<u64>()?,
        })
    }
}

#[derive(Debug)]
struct FileName {
    reference_to_parent_dir: u64,
    datetime_file_creation: u64,
    datetime_file_modification: u64,
    datetime_mft_modification: u64,
    datetime_file_reading: u64,
    file_size_allocated_on_disk: u64,
    real_file_size: u64,
    file_permission_flags: u32,
    extended_attributes_and_reparse: u32,
    name_size: u8,
    name: String,
    // 6 Bytes of padding
}

impl Readable for FileName {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        let reference_to_parent_dir = reader.read::<u64>()?;
        let datetime_file_creation = reader.read::<u64>()?;
        let datetime_file_modification = reader.read::<u64>()?;
        let datetime_mft_modification = reader.read::<u64>()?;
        let datetime_file_reading = reader.read::<u64>()?;
        let file_size_allocated_on_disk = reader.read::<u64>()?;
        let real_file_size = reader.read::<u64>()?;
        let file_permission_flags = reader.read::<u32>()?;
        let extended_attributes_and_reparse = reader.read::<u32>()?;
        let name_size = reader.read::<u8>()?;
        let name_bytes = reader.read_raw(name_size as usize * 2)?;
        // FIXME: File name is being interpreted wrongly, should be unicode
        let name = interpret_bytes_as_utf16(&name_bytes)
            .expect("Invalid utf16 bytes in attribute header.");
        // 6 Bytes of padding
        let _ = reader.read_byte_array::<6>()?;
        Ok(Self {
            reference_to_parent_dir,
            datetime_file_creation,
            datetime_file_modification,
            datetime_mft_modification,
            datetime_file_reading,
            file_size_allocated_on_disk,
            real_file_size,
            file_permission_flags,
            extended_attributes_and_reparse,
            name_size,
            name,
        })
    }
}

// https://sabercomlogica.com/en/ntfs-resident-and-no-named-attributes/
fn parse_mft(stream: &mut ByteStream, mft_lba: u64) -> io::Result<()> {
    stream.jump_to_sector(mft_lba)?;
    println!("Jump: {:#?}", stream.get_reader().stream_position());
    let mft_file_descriptor = stream.read::<MftFileDescriptor>()?;
    if *b"FILE" == mft_file_descriptor.signature {
        let attribute_offset =
            (mft_lba * SECTOR_SIZE as u64) + mft_file_descriptor.offset_fist_attribute as u64;
        stream.jump_to_byte(attribute_offset)?;
        println!("Here: {}", attribute_offset);
        let attribute_header = stream.read::<AttributeHeader>()?;
        println!("attribute_header: {:#?}", attribute_header);

        let standard_information = stream.read::<StandardInformation>()?;
        println!("Standard Info: {:#?}", standard_information);
        println!(
            "Unix Epoch: {}",
            (standard_information.datetime_file_creation - 116444736000000000) / 10000000
        );

        if let AttributeHeader::ResidentNoName { common_header, .. } = attribute_header {
            stream.jump_to_byte(attribute_offset + common_header.length as u64)?;
            let next_attribute_header = stream.read::<AttributeHeader>()?;
            println!("Next Attribute Header: {:#?}", next_attribute_header);
            // Increment by attribute_offset
            stream.jump_to_byte(attribute_offset + common_header.length as u64 + 24)?;
            // FIXME: File name is being read incorrectly, should be $MFT in this instance. The bytes
            // being read are correct but they are not being converted to strings correctly.
            let file_name = stream.read::<FileName>()?;
            println!("file_name: {:#?}", file_name);
        }
    } else {
        eprintln!("Bad file signature found in MFT file descriptor.");
    }

    Ok(())
}
