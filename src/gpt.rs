use std::{fmt::Display, io::{self, Bytes}, path::Path, string::{FromUtf8Error, FromUtf16Error}};

use crate::bytestream::{ByteStream, SECTOR_SIZE};


// https://www.ietf.org/rfc/rfc4122.txt
// 4.1.2.  Layout and Byte Order
#[derive(Debug)]
struct Guid {
    // The low field of the timestamp
    time_low: u32,
    // The middle field of the timestamp
    time_mid: u16,
    // The high field of the timestamp multiplexed with the version number
    time_high_and_version: u16,
    // The high field of the clock sequence multiplexed with the variant
    clock_seq_high_and_reserved: u8,
    // The low field of the clock sequence
    clock_seq_low: u8,
    // The spatially unique node identifier
    node_identifier: [u8; 6],
}

impl Guid {
    pub const fn new(bytes: [u8; 16]) -> Self {
        // The first three dash-delimited fields of the GUID are stored little endian, and the last two fields are not
        let time_low = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let time_mid = u16::from_le_bytes([bytes[4], bytes[5]]);
        let time_high_and_version = u16::from_le_bytes([bytes[6], bytes[7]]);
        let clock_seq_high_and_reserved = bytes[8];
        let clock_seq_low = bytes[9];
        let node_identifier = [
            bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ];

        Self {
            time_low,
            time_mid,
            time_high_and_version,
            clock_seq_high_and_reserved,
            clock_seq_low,
            node_identifier,
        }
    }
}

impl Display for Guid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let clock_seq = u16::from_be_bytes([self.clock_seq_high_and_reserved, self.clock_seq_low]);

        // Ignore first 2 bytes.
        let mut tmp_buffer = [0u8; 8];
        tmp_buffer[2..].copy_from_slice(&self.node_identifier);
        let node = u64::from_be_bytes(tmp_buffer);

        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            self.time_low, self.time_mid, self.time_high_and_version, clock_seq, node
        )
    }
}

#[test]
fn test_guid() {
    // https://developer.apple.com/library/archive/technotes/tn2166/_index.html#//apple_ref/doc/uid/DTS10003927-CH1-SECTION2
    let bytes: [u8; 16] = [
        0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9,
        0x3b,
    ];
    let guid = Guid::new(bytes);
    assert_eq!(format!("{}", guid), "c12a7328-f81f-11d2-ba4b-00a0c93ec93b")
}

#[derive(Debug)]
struct GptHeader {
    efi_part: [u8; 8],
    //Revision 1.0 (00h 00h 01h 00h) for UEFI 2.8
    revision: [u8; 4],
    // Header size in little endian (in bytes, usually 5Ch 00h 00h 00h or 92 bytes)
    header_size: [u8; 4],
    crc32: [u8; 4],
    // Reserved space of 0's
    reserved: [u8; 4],
    current_lba: [u8; 8],
    backup_lba: [u8; 8],
    first_usable_lba: [u8; 8],
    last_usable_lba: [u8; 8],
    disk_guid: [u8; 16],
    starting_lba_of_partition_entries: [u8; 8],
    number_partition_entries: [u8; 4],
    size_single_partition_entry: [u8; 4],
    crc32_partition_entries: [u8; 4],
}

impl GptHeader {
    fn signature(&self) -> Result<String, FromUtf8Error> {
        String::from_utf8(self.efi_part.to_vec())
    }

    fn header_size(&self) -> u32 {
        u32::from_le_bytes(self.header_size)
    }

    fn current_lba(&self) -> u64 {
        u64::from_le_bytes(self.current_lba)
    }

    fn backup_lba(&self) -> u64 {
        u64::from_le_bytes(self.backup_lba)
    }

    fn first_usable_lba(&self) -> u64 {
        u64::from_le_bytes(self.first_usable_lba)
    }

    fn last_usable_lba(&self) -> u64 {
        u64::from_le_bytes(self.last_usable_lba)
    }

    fn disk_guid(&self) -> Guid {
        Guid::new(self.disk_guid)
    }

    fn starting_lba_of_partition_entries(&self) -> u64 {
        u64::from_le_bytes(self.starting_lba_of_partition_entries)
    }

    fn number_partition_entries(&self) -> u32 {
        u32::from_le_bytes(self.number_partition_entries)
    }

    fn size_single_partition_entry(&self) -> u32 {
        u32::from_le_bytes(self.size_single_partition_entry)
    }
}

#[derive(Debug)]
struct GptPartitionTableEntry {
    partition_type_guid: [u8; 16],
    unique_partition_guid: [u8; 16],
    starting_lba: [u8; 8],
    ending_lba: [u8; 8],
    attribute_flags: [u8; 8],
    partition_name: [u8; 72],
}

impl GptPartitionTableEntry {
    fn is_empty(&self) -> bool {
        self.partition_type_guid.iter().all(|byte| *byte == 0)
            && self.unique_partition_guid.iter().all(|byte| *byte == 0)
            && self.starting_lba.iter().all(|byte| *byte == 0)
            && self.ending_lba.iter().all(|byte| *byte == 0)
            && self.attribute_flags.iter().all(|byte| *byte == 0)
            && self.partition_name.iter().all(|byte| *byte == 0)
    }

    fn starting_lba(&self) -> u64 {
        u64::from_le_bytes(self.starting_lba)
    }

    fn ending_lba(&self) -> u64 {
        u64::from_le_bytes(self.ending_lba)
    }

    fn partition_name(&self) -> Result<String, FromUtf16Error> {
        let num_bytes = self.partition_name.len();
        let mut unicode_symbols: Vec<u16> = Vec::with_capacity(num_bytes / 2);
        for index in (0..num_bytes).step_by(2) {
            // Order of top and bottom here is reversed since the bytes are in little endian
            let first = self.partition_name[index];
            let second = self.partition_name[index + 1];
            unicode_symbols.push(Self::bytes_to_u16(first, second));
        }

        String::from_utf16(&unicode_symbols)
    }

    fn bytes_to_u16(first: u8, second: u8) -> u16 {
        #[cfg(target_endian = "little")]
        {
            ((second as u16) << 8) | first as u16
        }
        #[cfg(target_endian = "big")]
        {
            ((first as u16) << 8) | second as u16
        }
    }
}

impl Display for GptPartitionTableEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "partition_type_guid: {}\n",
            Guid::new(self.partition_type_guid)
        )?;
        write!(
            f,
            "unique_partition_guid: {}\n",
            Guid::new(self.unique_partition_guid)
        )?;
        write!(f, "starting_lba: {}\n", self.starting_lba())?;
        write!(f, "ending_lba: {}\n", self.ending_lba())?;
        write!(f, "attribute_flags: {:#?}\n", self.attribute_flags)?;
        write!(f, "partition_name: {:#?}\n", self.partition_name())
    }
}

fn is_valid_crc32(path: &Path, header_size: u32, crc32: [u8; 4]) -> io::Result<bool> {
    let crc = u32::from_le_bytes(crc32);
    let mut stream = ByteStream::new(path, Some(0), 1)?;
    let mut header_bytes = stream.read_raw_bytes(header_size as usize);

    // CRC32 of header (offset +0 to +0x5b) in little endian, with this field zeroed during calculation
    header_bytes.splice(16..20, vec![0u8; 4]);
    Ok(calculate_crc32(header_bytes) == crc)
}

fn calculate_crc32(bytes: Vec<u8>) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;

    for mut byte in bytes {
        for _ in 0..8 {
            let tmp = (((byte as u32)) ^ crc) & 1;
            crc >>= 1;
            if tmp != 0 {
                crc = crc ^ 0xEDB88320;
            }
            byte >>= 1;
        }
    }
    !crc
}

pub fn parse_gpt(path: &Path) -> io::Result<()> {
    let mut stream = ByteStream::new(path, Some(0), 1)?;
    let header = stream.read::<GptHeader>()?;

    println!("Header guid: {}", header.disk_guid());
    println!("Header entry crc32: {:#?}", header.crc32_partition_entries);
    // 0x37 0x6D 0x91 0x1b
    if !is_valid_crc32(path, header.header_size(), header.crc32)? {
        // FIXME: Check backup header if crc32 fails.
        
    }

    let number_of_sectors = (header.number_partition_entries()
        * header.size_single_partition_entry())
        / SECTOR_SIZE as u32;


    let mut buffer: Vec<u8> = Vec::new();
    for index in 0..number_of_sectors {
        let sector_lba = header.starting_lba_of_partition_entries() + index as u64;
        let mut byte_stream = ByteStream::new(path, Some(0), sector_lba)?;
        
        buffer.extend(byte_stream.read_raw_bytes(SECTOR_SIZE));
        while !byte_stream.peek::<GptPartitionTableEntry>()?.is_empty() {
            let partition_table = byte_stream.read::<GptPartitionTableEntry>()?;
            println!("{}", partition_table);
        } 
    }
    println!("Here!: {:02x}", calculate_crc32(buffer));

    Ok(())
}
