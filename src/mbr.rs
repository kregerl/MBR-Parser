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
pub struct PartitionTable {
    bootable: u8,
    starting_chs: [u8; 3],
    partition_type: u8,
    ending_chs: [u8; 3],
    lba_start: [u8; 4],
    num_sectors: [u8; 4],
}

impl PartitionTable {
    fn is_empty(&self) -> bool {
        self.bootable == 0
            && self.starting_chs.iter().all(|byte| *byte == 0)
            && self.partition_type == 0
            && self.ending_chs.iter().all(|byte| *byte == 0)
            && self.lba_start.iter().all(|byte| *byte == 0)
            && self.num_sectors.iter().all(|byte| *byte == 0)
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

impl Display for PartitionTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}    {}({}, {}, {})    {}({}, {}, {})",
            self.bootable,
            self.lba_start(),
            PartitionTable::chs_cylinder(self.starting_chs),
            PartitionTable::chs_head(self.starting_chs),
            PartitionTable::chs_sector(self.starting_chs),
            self.num_sectors(),
            PartitionTable::chs_cylinder(self.ending_chs),
            PartitionTable::chs_head(self.ending_chs),
            PartitionTable::chs_sector(self.ending_chs)
        )

        // write!(f, "Bootable: {}\n", self.bootable)?;
        // // write!(f, "Starting CHS: {:#?}\n", self.starting_chs)?;
        // write!(f, "Starting:\n")?;
        // write!(
        //     f,
        //     " Cylinder: {}\n",
        //     PartitionTable::chs_cylinder(self.starting_chs)
        // )?;
        // write!(
        //     f,
        //     " Head: {}\n",
        //     PartitionTable::chs_head(self.starting_chs)
        // )?;
        // write!(
        //     f,
        //     " Sector: {}\n",
        //     PartitionTable::chs_sector(self.starting_chs)
        // )?;
        // write!(f, "Partition Type: {}\n", self.partition_type)?;
        // // write!(f, "Ending CHS: {:#?}\n", self.ending_chs)?;
        // write!(f, "Ending:\n")?;
        // write!(
        //     f,
        //     " Cylinder: {}\n",
        //     PartitionTable::chs_cylinder(self.ending_chs)
        // )?;
        // write!(f, " Head: {}\n", PartitionTable::chs_head(self.ending_chs))?;
        // write!(
        //     f,
        //     " Sector: {}\n",
        //     PartitionTable::chs_sector(self.ending_chs)
        // )?;
        // write!(f, "Starting LBA: {}\n", self.lba_start())?;
        // write!(f, "LBA # of Sectors: {}\n", self.num_sectors())
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

#[derive(Debug, Default)]
pub struct PartitionTableNode {
    pub partition_table: Option<PartitionTable>,
    pub children: Option<Vec<PartitionTableNode>>,
}

impl PartitionTableNode {
    fn new(partition_table: PartitionTable) -> Self {
        Self {
            partition_table: Some(partition_table),
            children: None,
        }
    }

    pub fn add_child_node(&mut self, partition_table_node: PartitionTableNode) {
        match &mut self.children {
            Some(children) => children.push(partition_table_node),
            None => self.children = Some(vec![partition_table_node]),
        }
    }
}

pub fn parse_sector(
    parent: &mut PartitionTableNode,
    path: &Path,
    image_offset_sectors: u64,
) -> io::Result<()> {
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
            break;
        }
        // Read table and discard zero'd out entries
        let table = stream.read::<PartitionTable>()?;
        if table.is_empty() {
            break;
        }
        // If table entry if an extended partition, follow it recusively
        let node = if table.partition_type == 0x05 {
            let start = table.lba_start() as u64;
            let mut node = PartitionTableNode::new(table);
            parse_sector(&mut node, path, start)?;
            node
        } else {
            PartitionTableNode::new(table)
        };
        parent.add_child_node(node);
    }
    Ok(())
}
