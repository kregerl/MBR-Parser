use std::{
    fmt::Display,
    io::{self, Bytes},
    path::Path,
    string::{FromUtf16Error, FromUtf8Error},
};

use prettytable::{row, Table};

use crate::bytestream::{self, ByteStream, SECTOR_SIZE};

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

impl ToString for Guid {
    fn to_string(&self) -> String {
        let clock_seq = u16::from_be_bytes([self.clock_seq_high_and_reserved, self.clock_seq_low]);

        // Ignore first 2 bytes.
        let mut tmp_buffer = [0u8; 8];
        tmp_buffer[2..].copy_from_slice(&self.node_identifier);
        let node = u64::from_be_bytes(tmp_buffer);

        let guid = format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            self.time_low, self.time_mid, self.time_high_and_version, clock_seq, node
        )
        .to_uppercase();
        guid
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
    assert_eq!(
        format!("{}", guid.to_string()),
        "C12A7328-F81F-11D2-BA4B-00A0C93EC93B"
    )
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
pub struct GptPartitionTableEntry {
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

    fn partition_type_guid(&self) -> Guid {
        Guid::new(self.partition_type_guid)
    }

    fn unique_partition_guid(&self) -> Guid {
        Guid::new(self.unique_partition_guid)
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
            Guid::new(self.partition_type_guid).to_string()
        )?;
        write!(
            f,
            "unique_partition_guid: {}\n",
            Guid::new(self.unique_partition_guid).to_string()
        )?;
        write!(f, "starting_lba: {}\n", self.starting_lba())?;
        write!(f, "ending_lba: {}\n", self.ending_lba())?;
        write!(f, "attribute_flags: {:#?}\n", self.attribute_flags)?;
        write!(f, "partition_name: {:#?}\n", self.partition_name())
    }
}

fn is_valid_header_crc32(path: &Path, header_size: u32, crc32: [u8; 4]) -> io::Result<bool> {
    let crc = u32::from_le_bytes(crc32);
    let mut stream = ByteStream::new(path, Some(0), 1)?;
    let mut header_bytes = stream.read_raw_bytes(header_size as usize)?;

    // CRC32 of header (offset +0 to +0x5b) in little endian, with this field zeroed during calculation
    header_bytes.splice(16..20, vec![0u8; 4]);
    Ok(calculate_crc32(header_bytes) == crc)
}

// https://lxp32.github.io/docs/a-simple-example-crc32-calculation/
fn calculate_crc32(bytes: Vec<u8>) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;

    for mut byte in bytes {
        for _ in 0..8 {
            let tmp = ((byte as u32) ^ crc) & 1;
            crc >>= 1;
            if tmp != 0 {
                crc = crc ^ 0xEDB88320;
            }
            byte >>= 1;
        }
    }
    !crc
}

pub fn parse_gpt(path: &Path) -> io::Result<Vec<GptPartitionTableEntry>> {
    let mut stream = ByteStream::new(path, Some(0), 1)?;
    let header = stream.read::<GptHeader>()?;

    println!("Header guid: {}", header.disk_guid().to_string());
    println!();
    if !is_valid_header_crc32(path, header.header_size(), header.crc32)? {
        // FIXME: Check backup header if crc32 fails.
    }

    let number_of_sectors = (header.number_partition_entries()
        * header.size_single_partition_entry())
        / SECTOR_SIZE as u32;

    let start = header.starting_lba_of_partition_entries() as usize;
    let end = (header.starting_lba_of_partition_entries() + number_of_sectors as u64) as usize;
    let buffer = ByteStream::read_disk_image(path, start, end)?;

    if calculate_crc32(buffer) != u32::from_le_bytes(header.crc32_partition_entries) {
        // FIXME: Check backup header if crc32 fails.
    }

    let mut partition_table = Vec::new();
    for index in 0..number_of_sectors {
        let sector_lba = header.starting_lba_of_partition_entries() + index as u64;
        let mut byte_stream = ByteStream::new(path, Some(0), sector_lba)?;

        while !byte_stream.peek::<GptPartitionTableEntry>()?.is_empty() {
            let partition_table_entry = byte_stream.read::<GptPartitionTableEntry>()?;
            partition_table.push(partition_table_entry);
        }
    }

    Ok(partition_table)
}

pub fn display_gpt(partition_table_entries: Vec<GptPartitionTableEntry>) {
    let mut table = Table::new();
    // TODO: Partition Attributes
    // https://en.wikipedia.org/wiki/GUID_Partition_Table#:~:text=The%20GUID%20Partition%20Table%20(GPT,globally%20unique%20identifiers%20(GUIDs).
    table.add_row(row![
        "LBA Starting Sector",
        "LBA Ending Sector",
        "Total Sectors",
        "Size (MB)",
        "Partition Type"
    ]);
    for partition_table_entry in partition_table_entries {
        let total_sectors = partition_table_entry.ending_lba() - partition_table_entry.starting_lba() + 1;
        table.add_row(row![
            partition_table_entry.starting_lba(),
            partition_table_entry.ending_lba(),
            total_sectors,
            ((total_sectors * SECTOR_SIZE as u64) as f64 / 1048576 as f64).round(),
            lookup_partition_type(partition_table_entry.partition_type_guid())
        ]);
    }
    table.printstd();
}

fn lookup_partition_type(partition_type: Guid) -> String {
    match partition_type.to_string().as_str() {
        "C12A7328-F81F-11D2-BA4B-00A0C93EC93B" => "EFI System",
        "024DEE41-33E7-11D3-9D69-0008C781F39F" => "MBR partition scheme",
        "D3BFE2DE-3DAF-11DF-BA40-E3A556D89593" => "Intel Fast Flash",
        "21686148-6449-6E6F-744E-656564454649" => "BIOS boot",
        "F4019732-066E-4E12-8273-346C5641494F" => "Sony boot partition",
        "BFBFAFE7-A34F-448A-9A5B-6213EB736C22" => "Lenovo boot partition",
        "9E1A2D38-C612-4316-AA26-8B49521E5A8B" => "PowerPC PReP boot",
        "7412F7D5-A156-4B13-81DC-867174929325" => "ONIE boot",
        "D4E6E2CD-4469-46F3-B5CB-1BFF57AFC149" => "ONIE config",
        "E3C9E316-0B5C-4DB8-817D-F92DF00215AE" => "Microsoft reserved",
        "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7" => "Microsoft basic data",
        "5808C8AA-7E8F-42E0-85D2-E1E90434CFB3" => "Microsoft LDM metadata",
        "AF9B60A0-1431-4F62-BC68-3311714A69AD" => "Microsoft LDM data",
        "DE94BBA4-06D1-4D40-A16A-BFD50179D6AC" => "Windows recovery environment",
        "37AFFC90-EF7D-4E96-91C3-2D7AE055B174" => "IBM General Parallel Fs",
        "E75CAF8F-F680-4CEE-AFA3-B001E56EFC2D" => "Microsoft Storage Spaces",
        "75894C1E-3AEB-11D3-B7C1-7B03A0000000" => "HP-UX data",
        "E2A1E728-32E3-11D6-A682-7B03A0000000" => "HP-UX service",
        "0657FD6D-A4AB-43C4-84E5-0933C84B4F4F" => "Linux swap",
        "0FC63DAF-8483-4772-8E79-3D69D8477DE4" => "Linux filesystem",
        "3B8F8425-20E0-4F3B-907F-1A25A76F98E8" => "Linux server data",
        "44479540-F297-41B2-9AF7-D131D5F0458A" => "Linux root (x86)",
        "69DAD710-2CE4-4E3C-B16C-21A1D49ABED3" => "Linux root (ARM)",
        "4F68BCE3-E8CD-4DB1-96E7-FBCAF984B709" => "Linux root (x86-64)",
        "B921B045-1DF0-41C3-AF44-4C6F280D3FAE" => "Linux root (ARM-64)",
        "993D8D3D-F80E-4225-855A-9DAF8ED7EA97" => "Linux root  (IA-64)",
        "8DA63339-0007-60C0-C436-083AC8230908" => "Linux reserved",
        "933AC7E1-2EB4-4F13-B844-0E14E2AEF915" => "Linux home",
        "A19D880F-05FC-4D3B-A006-743F0F84911E" => "Linux RAID",
        "BC13C2FF-59E6-4262-A352-B275FD6F7172" => "Linux extended boot",
        "E6D6D379-F507-44C2-A23C-238F2A3DF928" => "Linux LVM",
        "516E7CB4-6ECF-11D6-8FF8-00022D09712B" => "FreeBSD data",
        "83BD6B9D-7F41-11DC-BE0B-001560B84F0F" => "FreeBSD boot",
        "516E7CB5-6ECF-11D6-8FF8-00022D09712B" => "FreeBSD swap",
        "516E7CB6-6ECF-11D6-8FF8-00022D09712B" => "FreeBSD UFS",
        "516E7CBA-6ECF-11D6-8FF8-00022D09712B" => "FreeBSD ZFS",
        "516E7CB8-6ECF-11D6-8FF8-00022D09712B" => "FreeBSD Vinum",
        "48465300-0000-11AA-AA11-00306543ECAC" => "Apple HFS/HFS+",
        "55465300-0000-11AA-AA11-00306543ECAC" => "Apple UFS",
        "52414944-0000-11AA-AA11-00306543ECAC" => "Apple RAID",
        "52414944-5F4F-11AA-AA11-00306543ECAC" => "Apple RAID offline",
        "426F6F74-0000-11AA-AA11-00306543ECAC" => "Apple boot",
        "4C616265-6C00-11AA-AA11-00306543ECAC" => "Apple label",
        "5265636F-7665-11AA-AA11-00306543ECAC" => "Apple TV recovery",
        "53746F72-6167-11AA-AA11-00306543ECAC" => "Apple Core storage",
        "6A82CB45-1DD2-11B2-99A6-080020736631" => "Solaris boot",
        "6A85CF4D-1DD2-11B2-99A6-080020736631" => "Solaris root",
        "6A898CC3-1DD2-11B2-99A6-080020736631" => "Solaris /usr & Apple ZFS",
        "6A87C46F-1DD2-11B2-99A6-080020736631" => "Solaris swap",
        "6A8B642B-1DD2-11B2-99A6-080020736631" => "Solaris backup",
        "6A8EF2E9-1DD2-11B2-99A6-080020736631" => "Solaris /var",
        "6A90BA39-1DD2-11B2-99A6-080020736631" => "Solaris /home",
        "6A9283A5-1DD2-11B2-99A6-080020736631" => "Solaris alternate sector",
        "6A945A3B-1DD2-11B2-99A6-080020736631" => "Solaris reserved 1",
        "6A9630D1-1DD2-11B2-99A6-080020736631" => "Solaris reserved 2",
        "6A980767-1DD2-11B2-99A6-080020736631" => "Solaris reserved 3",
        "6A96237F-1DD2-11B2-99A6-080020736631" => "Solaris reserved 4",
        "6A8D2AC7-1DD2-11B2-99A6-080020736631" => "Solaris reserved 5",
        "49F48D32-B10E-11DC-B99B-0019D1879648" => "NetBSD swap",
        "49F48D5A-B10E-11DC-B99B-0019D1879648" => "NetBSD FFS",
        "49F48D82-B10E-11DC-B99B-0019D1879648" => "NetBSD LFS",
        "2DB519C4-B10E-11DC-B99B-0019D1879648" => "NetBSD concatenated",
        "2DB519EC-B10E-11DC-B99B-0019D1879648" => "NetBSD encrypted",
        "49F48DAA-B10E-11DC-B99B-0019D1879648" => "NetBSD RAID",
        "FE3A2A5D-4F32-41A7-B725-ACCC3285A309" => "ChromeOS kernel",
        "3CB8E202-3B7E-47DD-8A3C-7FF2A13CFCEC" => "ChromeOS root fs",
        "2E0A753D-9E48-43B0-8337-B15192CB1B5E" => "ChromeOS reserved",
        "85D5E45A-237C-11E1-B4B3-E89A8F7FC3A7" => "MidnightBSD data",
        "85D5E45E-237C-11E1-B4B3-E89A8F7FC3A7" => "MidnightBSD boot",
        "85D5E45B-237C-11E1-B4B3-E89A8F7FC3A7" => "MidnightBSD swap",
        "0394EF8B-237E-11E1-B4B3-E89A8F7FC3A7" => "MidnightBSD UFS",
        "85D5E45D-237C-11E1-B4B3-E89A8F7FC3A7" => "MidnightBSD ZFS",
        "85D5E45C-237C-11E1-B4B3-E89A8F7FC3A7" => "MidnightBSD Vinum",
        "45B0969E-9B03-4F30-B4C6-B4B80CEFF106" => "Ceph Journal",
        "45B0969E-9B03-4F30-B4C6-5EC00CEFF106" => "Ceph Encrypted Journal",
        "4FBD7E29-9D25-41B8-AFD0-062C0CEFF05D" => "Ceph OSD",
        "4FBD7E29-9D25-41B8-AFD0-5EC00CEFF05D" => "Ceph crypt OSD",
        "89C57F98-2FE5-4DC0-89C1-F3AD0CEFF2BE" => "Ceph disk in creation",
        "89C57F98-2FE5-4DC0-89C1-5EC00CEFF2BE" => "Ceph crypt disk in creation",
        "AA31E02A-400F-11DB-9590-000C2911D1B8" => "VMware VMFS",
        "9D275380-40AD-11DB-BF97-000C2911D1B8" => "VMware Diagnostic",
        "381CFCCC-7288-11E0-92EE-000C2911D0B2" => "VMware Virtual SAN",
        "77719A0C-A4A0-11E3-A47E-000C29745A24" => "VMware Virsto",
        "9198EFFC-31C0-11DB-8F78-000C2911D1B8" => "VMware Reserved",
        "824CC7A0-36A8-11E3-890A-952519AD3F61" => "OpenBSD data",
        "CEF5A9AD-73BC-4601-89F3-CDEEEEE321A1" => "QNX6 file system",
        "C91818F9-8025-47AF-89D2-F030D7000C2C" => "Plan 9 partition",
        "5B193300-FC78-40CD-8002-E86C45580B47" => "HiFive Unleashed FSBL",
        "2E54B353-1271-4842-806F-E436D6AF6985" => "HiFive Unleashed BBL",
        _ => "Unknown Partition Type",
    }
    .into()
}
