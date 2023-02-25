use std::{
    fs::File,
    io::{self, Cursor, Read, Seek, SeekFrom},
    mem::{self, MaybeUninit},
    path::Path,
    slice,
};
pub const SECTOR_SIZE: usize = 512;

// FIXME: This bytestream will not work correctly with big endian systems.
pub struct ByteStream {
    bytes: Cursor<Vec<u8>>,
    index: usize,
}

impl From<Vec<u8>> for ByteStream {
    fn from(readable: Vec<u8>) -> Self {
        Self {
            bytes: Cursor::new(readable),
            index: 0
        }
    }
}

impl ByteStream {
    /// `Path` to file to read a sector(512 bytes starting at `starting_index`) starting from `image_offset_sectors`
    pub fn new(
        path: &Path,
        starting_index: Option<usize>,
        image_offest_sectors: u64,
    ) -> io::Result<Self> {

        let start_sector = image_offest_sectors as usize;
        Ok(Self {
            bytes: Cursor::new(Self::read_disk_image(path, start_sector, start_sector + SECTOR_SIZE)?),
            index: if let Some(index) = starting_index {
                index
            } else {
                0
            },
        })
    }

    /// Reads the first sector from an image (little-endian)
    pub fn read_disk_image(image_path: &Path, from_sector: usize, to_sector: usize) -> io::Result<Vec<u8>> {
        let mut image_file = File::open(image_path)?;
        let mut buffer = vec![0u8; (to_sector - from_sector) * SECTOR_SIZE];
        image_file.seek(SeekFrom::Start((from_sector * SECTOR_SIZE) as u64))?;
        image_file.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    /// Get the next T from bytes without advancing
    pub fn peek<T>(&mut self) -> io::Result<T> {
        self.read_impl(false)
    }

    /// Advance and get the next T from bytes  
    pub fn read<T>(&mut self) -> io::Result<T> {
        self.read_impl(true)
    }

    /// Read bytes into a vec starting at `index` until `index + amount`
    pub fn read_raw_bytes(&mut self, amount: usize) -> io::Result<Vec<u8>> {
        let mut buffer = vec![Default::default(); amount];
        self.bytes.read_exact(&mut buffer)?;
        Ok(buffer)
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
