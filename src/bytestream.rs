use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
};
pub const SECTOR_SIZE: usize = 512;

pub trait Readable {
    fn read(reader: &mut BufReader<File>) -> io::Result<Self>
    where
        Self: Sized;
}

pub struct ByteStream {
    reader: BufReader<File>,
}

impl ByteStream {
    pub fn new(path: &Path) -> io::Result<Self> {
        let reader = BufReader::new(File::open(path)?);
        Ok(Self { reader })
    }

    pub fn read_raw(&mut self, amount: usize) -> io::Result<Vec<u8>> {
        let mut buffer = vec![0u8; amount];
        self.reader.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    pub fn read_raw_from_sectors(&mut self, from: usize, amount: usize) -> io::Result<Vec<u8>> {
        let current = self.reader.seek(SeekFrom::Current(0))?;
        let mut buffer = vec![0u8; amount * SECTOR_SIZE];
        let _ = self.reader.seek(SeekFrom::Start((from * SECTOR_SIZE) as u64))?;
        self.reader.read_exact(&mut buffer)?;
        let _ = self.reader.seek(SeekFrom::Start(current))?;
        Ok(buffer)
    }

    pub fn read_raw_from_bytes(&mut self, from: usize, amount: usize) -> io::Result<Vec<u8>> {
        let current = self.reader.seek(SeekFrom::Current(0))?;
        let mut buffer = vec![0u8; amount];
        let _ = self.reader.seek(SeekFrom::Start(from as u64))?;
        self.reader.read_exact(&mut buffer)?;
        let _ = self.reader.seek(SeekFrom::Start(current))?;
        Ok(buffer)
    }

    pub fn read<T>(&mut self) -> io::Result<T>
    where
        T: Readable,
    {
        T::read(&mut self.reader)
    }

    pub fn jump_to_sector(&mut self, amount: u64) -> io::Result<()> {
        self.jump_to_byte(amount * SECTOR_SIZE as u64)
    }

    pub fn jump_to_byte(&mut self, amount: u64) -> io::Result<()> {
        self.reader.seek(SeekFrom::Start(amount))?;
        Ok(())
    }
}