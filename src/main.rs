use std::{
    fs::{File},
    io::{self, Cursor, Read},
    mem::{self, MaybeUninit},
    path::Path,
    slice, fmt::Display,
};

const BOOTSTRAPER_LENGTH: usize = 446;
const MBR_SIZE: usize = 512;
const BOOT_SIGNATURE: [u8; 2] = [0x55, 0xAA];
const CHS_SECTOR_BIT_SIZE: u8 = 6;
const FIRST_TWO_BIT_MASK: u16 = 0b11000000;

#[derive(Debug)]
struct PartitionTable {
    bootable: u8,
    starting_chs: [u8; 3],
    partition_type: u8,
    ending_chs: [u8; 3],
    lba_start: [u8; 4],
    num_sectors: [u8; 4],
}

impl PartitionTable {
    
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
        write!(f, "Bootable: {}\n", self.bootable)?;
        // write!(f, "Starting CHS: {:#?}\n", self.starting_chs)?;
        write!(f, "Starting:\n")?;
        write!(f, " Cylinder: {}\n", PartitionTable::chs_cylinder(self.starting_chs))?;
        write!(f, " Head: {}\n", PartitionTable::chs_head(self.starting_chs))?;
        write!(f, " Sector: {}\n", PartitionTable::chs_sector(self.starting_chs))?;
        write!(f, "Partition Type: {}\n", self.partition_type)?;
        // write!(f, "Ending CHS: {:#?}\n", self.ending_chs)?;
        write!(f, "Ending:\n")?;
        write!(f, " Cylinder: {}\n", PartitionTable::chs_cylinder(self.ending_chs))?;
        write!(f, " Head: {}\n", PartitionTable::chs_head(self.ending_chs))?;
        write!(f, " Sector: {}\n", PartitionTable::chs_sector(self.ending_chs))?;
        write!(f, "Starting LBA: {}\n", self.lba_start())?;
        write!(f, "LBA # of Sectors: {}\n", self.num_sectors())
    }
}

struct ByteStream {
    bytes: Cursor<Vec<u8>>,
    index: usize,
}

impl ByteStream {
    pub fn new(path: &Path) -> io::Result<Self> {
        Ok(Self {
            bytes: Cursor::new(read_disk_image(path)?),
            index: BOOTSTRAPER_LENGTH as usize,
        })
    }

    pub fn peek<T>(&mut self) -> io::Result<T> {
        self.read_impl(false)
    }

    pub fn read<T>(&mut self) -> io::Result<T> {
        self.read_impl(true)
    }

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

fn main() {
    if let Err(e) = parse_mbr(&Path::new("./one-gb.dd")) {
        eprintln!("Error parsing MBR: {}", e);
    }
}

/// Parse the MBR
fn parse_mbr(path: &Path) -> io::Result<()> {
    let mut stream = ByteStream::new(path)?;

    while stream.peek::<[u8; 2]>()? != BOOT_SIGNATURE {
        println!("--------------------------");
        let table = stream.read::<PartitionTable>()?;
        println!("Partition Table: {}", table);
    }
    Ok(())
}

/// Reads the first sector from an image (little-endian)
fn read_disk_image(image_path: &Path) -> io::Result<Vec<u8>> {
    let mut image_file = File::open(image_path)?;
    let mut buffer = vec![0u8; MBR_SIZE];
    image_file.read_exact(&mut buffer)?;
    Ok(buffer)
}
