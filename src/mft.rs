use crate::{
    bytestream::{interpret_bytes_as_utf16, ByteStream, Readable, SECTOR_SIZE},
    mbr::{MbrPartitionTableEntryNode, BOOT_SIGNATURE},
};
use chrono::{DateTime, Local};
use std::{
    fmt::Display,
    io::{self, Seek},
    path::Path,
    string::FromUtf8Error,
    time::{Duration, UNIX_EPOCH},
};

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
    offset_first_attribute: u16,
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
            offset_first_attribute: reader.read::<u16>()?,
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

impl AttributeHeader {
    pub fn common_header(&self) -> &CommonAttributeHeader {
        match self {
            AttributeHeader::ResidentNoName { common_header, .. } => common_header,
            AttributeHeader::ResidentNamed { common_header, .. } => common_header,
            AttributeHeader::NonResidentNoName { common_header, .. } => common_header,
            AttributeHeader::NonResidentNamed { common_header, .. } => common_header,
        }
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
struct NtfsDatetime {
    datetime: DateTime<Local>,
}

impl Readable for NtfsDatetime {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        let ole2_timestamp = reader.read::<u64>()?;
        let timestamp_unix_epoch = (ole2_timestamp - 116444736000000000) / 10000000;
        let datetime =
            DateTime::<Local>::from(UNIX_EPOCH + Duration::from_secs(timestamp_unix_epoch));
        Ok(Self { datetime })
    }
}

#[derive(Debug)]
struct StandardInformation {
    datetime_file_creation: NtfsDatetime,
    datetime_file_modification: NtfsDatetime,
    datetime_mft_modification: NtfsDatetime,
    datetime_file_reading: NtfsDatetime,
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
            datetime_file_creation: reader.read::<NtfsDatetime>()?,
            datetime_file_modification: reader.read::<NtfsDatetime>()?,
            datetime_mft_modification: reader.read::<NtfsDatetime>()?,
            datetime_file_reading: reader.read::<NtfsDatetime>()?,
            file_permission_flags: reader.read::<u32>()?,
            maximum_number_versions: reader.read::<u32>()?,
            version_number: reader.read::<u64>()?,
        })
    }
}

#[derive(Debug)]
struct FileName {
    reference_to_parent_dir: u64,
    datetime_file_creation: NtfsDatetime,
    datetime_file_modification: NtfsDatetime,
    datetime_mft_modification: NtfsDatetime,
    datetime_file_reading: NtfsDatetime,
    file_size_allocated_on_disk: u64,
    real_file_size: u64,
    file_permission_flags: u32,
    extended_attributes_and_reparse: u32,
    name_size: u8,
    namespace: u8,
    name: String,
    // 6 Bytes of padding
}

impl Readable for FileName {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        let reference_to_parent_dir = reader.read::<u64>()?;
        let datetime_file_creation = reader.read::<NtfsDatetime>()?;
        let datetime_file_modification = reader.read::<NtfsDatetime>()?;
        let datetime_mft_modification = reader.read::<NtfsDatetime>()?;
        let datetime_file_reading = reader.read::<NtfsDatetime>()?;
        let file_size_allocated_on_disk = reader.read::<u64>()?;
        let real_file_size = reader.read::<u64>()?;
        let file_permission_flags = reader.read::<u32>()?;
        let extended_attributes_and_reparse = reader.read::<u32>()?;
        let name_size = reader.read::<u8>()?;
        let namespace = reader.read::<u8>()?;
        let name_bytes = reader.read_raw(name_size as usize * 2)?;
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
            namespace,
            name,
        })
    }
}

#[derive(Debug)]
struct DataRun {
    dataruns: Vec<u8>,
}

impl Readable for DataRun {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        let dataruns = reader.read_until(0x00)?;
        Ok(Self { dataruns })
    }
}

// https://sabercomlogica.com/en/ntfs-resident-and-no-named-attributes/
fn parse_mft(stream: &mut ByteStream, mft_lba: u64) -> io::Result<()> {
    println!("LBA: {}", mft_lba);
    let mut starting_offset = mft_lba * SECTOR_SIZE as u64;
    let mut file_name_attrs: Vec<FileName> = Vec::new();
    loop {
        stream.jump_to_byte(starting_offset)?;
        println!("Jump: {:#?}", stream.get_reader().stream_position());
        let mft_file_descriptor = stream.read::<MftFileDescriptor>()?;
        if *b"FILE" == mft_file_descriptor.signature {
            let attribute_start_offset =
                starting_offset + mft_file_descriptor.offset_first_attribute as u64;
            let mut attribute_offset: u64 = 0;

            while stream.peek::<u32>()? != u32::MAX {
                stream.jump_to_byte(attribute_start_offset + attribute_offset)?;
                let attribute_header = stream.read::<AttributeHeader>()?;
                let common_header = attribute_header.common_header();

                // FIXME account for update sequences: https://stackoverflow.com/questions/55126151/ntfs-mft-datarun
                // https://www.youtube.com/watch?v=6WFUM5eViIk
                match common_header.attribute_type {
                    0x10 => {
                        let standard_information = stream.read::<StandardInformation>()?;
                        println!("standard_information: {:#?}", standard_information);
                    }
                    0x20 => {}
                    0x30 => {
                        let file_name = stream.read::<FileName>()?;
                        println!("file_name: {:#?}", file_name);
                        file_name_attrs.push(file_name);
                    }
                    0x40 => {
                        //FIXME: $OJECT_ID
                        todo!("$OJECT_ID")
                    }
                    0x50 => {
                        //FIXME: Read $SECURITY_DEXCRIPTOR
                        todo!("$SECURITY_DEXCRIPTOR")
                    }
                    0x60 => {
                        //FIXME: $VOLUMNE_NAME
                        todo!("$VOLUMNE_NAME")
                    }
                    0x70 => {
                        //FIXME: $VOLUMNE_INFORMATION
                        todo!("$VOLUMNE_INFORMATION")
                    }
                    0x80 => {
                        //FIXME: $DATA
                        eprintln!(
                            "Ignored attribute_header: {:#?} of type {:#02x}",
                            attribute_header, common_header.attribute_type
                        );
                        todo!("$DATA")
                    }
                    0x90 => {
                        //FIXME: $INDEX_ROOT
                        todo!("$INDEX_ROOT")
                    }
                    0xA0 => {
                        //FIXME: $INDEX_ALLOCATION
                        todo!("$INDEX_ALLOCATION")
                    }
                    0xB0 => {
                        //FIXME: $BITMAP
                        todo!("$BITMAP")
                    }
                    0xC0 => {
                        //FIXME: $REPARSE_POINT
                        todo!("$REPARSE_POINT")
                    }
                    0xD0 => {
                        //FIXME: $EA_INFORMATION
                        todo!("$EA_INFORMATION")
                    }
                    0xE0 => {
                        //FIXME: $EA
                        todo!("$EA")
                    }
                    0xF0 => {
                        //FIXME: $PROPERTY_SET
                        todo!("$PROPERTY_SET")
                    }
                    0x100 => {
                        //FIXME: $LOGGED_UTILITY_STREAM
                        todo!("$LOGGED_UTILITY_STREAM")
                    }
                    _ => {
                        eprintln!(
                            "Ignored attribute_header: {:#?} of type {:#02x}",
                            attribute_header, common_header.attribute_type
                        );
                        starting_offset += mft_file_descriptor.space_allocated as u64;
                        break;
                    }
                }
                attribute_offset += common_header.length as u64;
            }
            // let attribute_offset =
            //     (mft_lba * SECTOR_SIZE as u64) + mft_file_descriptor.offset_fist_attribute as u64;
            // stream.jump_to_byte(attribute_offset)?;
            // println!("Here: {}", attribute_offset);
            // let attribute_header = stream.read::<AttributeHeader>()?;
            // println!("attribute_header: {:#?}", attribute_header);

            // let standard_information = stream.read::<StandardInformation>()?;
            // println!("Standard Info: {:#?}", standard_information);
            // println!(
            //     "Unix Epoch: {}",
            //     (standard_information.datetime_file_creation - 116444736000000000) / 10000000
            // );

            // if let AttributeHeader::ResidentNoName { common_header, resident_header } = attribute_header {
            //     stream.jump_to_byte(attribute_offset + common_header.length as u64)?;
            //     let next_attribute_header = stream.read::<AttributeHeader>()?;
            //     println!("Next Attribute Header: {:#?}", next_attribute_header);
            //     // Increment by attribute_offset
            //     stream.jump_to_byte(attribute_offset + common_header.length as u64 + resident_header.attribute_offset as u64)?;
            //     let file_name = stream.read::<FileName>()?;
            //     println!("file_name: {:#?}", file_name);
            //     println!("Next header: {:#?}", stream.read::<AttributeHeader>()?);
            //     let x = stream.read_until(0x00)?;
            //     // Discard padding to byte align the position
            //     let _ = stream.read_byte_array::<3>()?;
            //     // TODO: <356929856> Read the dataruns, stopping at 0x00 0x00 0x00
            //     println!("Data Runs: {:#?}", x);
            //     println!("Next header3: {:#?}", stream.read::<AttributeHeader>()?);
            //     println!("Jump: {:#?}", stream.get_reader().stream_position());
            //     let x = stream.read_until(0x00)?;
            //     println!("Data Runs: {:#?}", x);
            // }
        } else {
            eprintln!("Bad file signature found in MFT file descriptor.");
            break;
        }
    }
    println!("Offset: {:#?}", stream.get_reader().stream_position());
    for file_name in file_name_attrs {
        println!("- {}", file_name.name);
    }

    Ok(())
}

// struct AttributeParseError;

// impl fmt::Display for AttributeParseError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "Error parsing attribute, unknown attribute type")
//     }
// }

// fn parse_attribute_type() -> Result<(), > {

// }

#[test]
fn datarun_test() {
    // let datarun: [u8; 8] = [0x21, 0x18, 0x34, 0x56, 0x00, 0x00, 0x00, 0x00];
    // let datarun: [u8; 8] = [0x31, 0x01, 0x41, 0x00, 0x01, 0x00, 0x00, 0x00];
    let datarun: [u8; 8] = [0x31, 0x40, 0x55, 0x4f, 0x01, 0x00, 0x00, 0x00];
    //356864000
    //356929536
    let mut length: u64 = 0;
    let mut offset: i64 = 0;

    let high_nibble = (datarun[0] & 0b11110000) >> 4;
    let low_nibble = datarun[0] & 0b00001111;

    for i in 0..low_nibble as usize {
        length |= (datarun[1 + i] as u64) << (i * 8);
    }

    for i in 0..high_nibble as usize {
        offset |= (datarun[1 + low_nibble as usize + i] as i64) << (i * 8);
    }

    println!("Here: {:#04x}", offset & (1 << (high_nibble * 8 - 1)));
    if offset & (1 << (high_nibble * 8 - 1)) > 0 {
        for i in 0..high_nibble as usize {
            offset |= (0xFF as i64) << (i * 8);
        }
    }

    println!("Length: {:#04x}", length);
    println!("Offset: {:#04x}", offset);

}