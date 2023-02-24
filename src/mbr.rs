use std::{
    fmt::Display,
    fs::File,
    io::{self, Cursor, Read, Seek, SeekFrom},
    mem::{self, MaybeUninit},
    path::Path,
    slice,
};

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

    pub fn is_extended_partition(&self) -> bool {
        self.partition_type == 0x05
    }

    fn lba_start(&self) -> u32 {
        u32::from_le_bytes(self.lba_start)
    }

    fn num_sectors(&self) -> u32 {
        u32::from_le_bytes(self.num_sectors)
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

impl Display for PartitionTableEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let start_chs = format!(
            "(C:{}, H:{}, S:{})",
            PartitionTableEntry::chs_cylinder(self.starting_chs),
            PartitionTableEntry::chs_head(self.starting_chs),
            PartitionTableEntry::chs_sector(self.starting_chs)
        );
        let end_chs = format!(
            "(C:{}, H:{}, S:{})",
            PartitionTableEntry::chs_cylinder(self.ending_chs),
            PartitionTableEntry::chs_head(self.ending_chs),
            PartitionTableEntry::chs_sector(self.ending_chs)
        );

        write!(
            f,
            "| {:<5} | {:<10} | {:<12} | {:<22} | {:<12} | {:<22} | {:<12} | ",
            self.partition_type,
            self.bootable,
            self.lba_start(),
            start_chs,
            self.lba_start() + self.num_sectors(),
            end_chs,
            self.num_sectors(),
        )
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

#[derive(Debug)]
pub struct PartitionTableNode {
    pub partition_table_entry: PartitionTableEntry,
    pub children: Option<Vec<PartitionTableNode>>,
}

impl PartitionTableNode {
    fn new(partition_table_entry: PartitionTableEntry) -> Self {
        Self {
            partition_table_entry: partition_table_entry,
            children: None,
        }
    }

    pub fn add_child_node(&mut self, partition_table_entry_node: PartitionTableNode) {
        match &mut self.children {
            Some(children) => children.push(partition_table_entry_node),
            None => self.children = Some(vec![partition_table_entry_node]),
        }
    }

    pub fn add_children(&mut self, partition_table_nodes: Vec<PartitionTableNode>) {
        for node in partition_table_nodes {
            self.add_child_node(node);
        }
    }
}

fn print_partition_table_entry(
    partition_table_entry: &PartitionTableEntry,
    image_offset_sectors: u64,
) {
    let start_lba = partition_table_entry.lba_start() as u64 + image_offset_sectors;
    println!(
        "| {:<4} | {:<4} | {:<12} | {:<12} | {:<12} | {} + {} = {}",
        partition_table_entry.partition_type,
        partition_table_entry.bootable,
        start_lba,
        start_lba + partition_table_entry.num_sectors() as u64 - 1,
        partition_table_entry.num_sectors(),
        image_offset_sectors,
        partition_table_entry.lba_start(),
        image_offset_sectors + partition_table_entry.lba_start() as u64
    );
}


// TODO: switch this recursive function out for a simpler approach. First parse the 4 entry mbr and IF there is an EBR then we recursively follow
// TODO: that EBR with the first_ebr_lba set to the first EBR's starting LBA. That way the next EBR's can be found using `first_ebr_lba + ebr.lba_start()`. 
// TODO: first_ebr_lba should never be updated.

/// first_ebr_lba :: The First extended block's Logical Block Address
pub fn parse_sector(path: &Path, is_first: bool, image_offset_sectors: u64, first_ebr_lba: u64) -> io::Result<()> {
    let mut stream = ByteStream::new(
        path,
        Some(BOOTSTRAPER_LENGTH as usize),
        image_offset_sectors,
    )?;

    // Stop the sector at BOOT_SIGNATURE
    while stream.peek::<[u8; 2]>()? != BOOT_SIGNATURE {
        let peek_byte = stream.peek::<u8>()?;
        // https://en.wikipedia.org/wiki/Master_boot_record#PTE:
        // MBRs only accept 0x80, 0x00 means inactive, and 0x01–0x7F stand for invalid
        if peek_byte != 0x00 && peek_byte != 0x80 && (0x01..0x7F).contains(&peek_byte) {
            eprintln!("Skipping... {}", peek_byte);
            break;
        }
        // Read table and discard zero'd out entries
        let partition_table_entry = stream.read::<PartitionTableEntry>()?;
        if partition_table_entry.is_empty() {
            break;
        }

        if partition_table_entry.is_extended_partition() {
            let start = partition_table_entry.lba_start() as u64;
            if is_first {
                print_partition_table_entry(&partition_table_entry, image_offset_sectors);
                parse_sector(path, true, start + image_offset_sectors, first_ebr_lba)?;
            } else {
                parse_sector(path, true, start + image_offset_sectors, first_ebr_lba)?;
            }
        } else {
            print_partition_table_entry(&partition_table_entry, image_offset_sectors);
        }
    }
    Ok(())
}
