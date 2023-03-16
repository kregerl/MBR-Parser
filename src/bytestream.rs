use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
    string::FromUtf16Error,
};

use byteorder::{ByteOrder, ReadBytesExt};
pub const SECTOR_SIZE: usize = 512;

// FIXME: Remove `Readable` impls for numbers and replace with `ReadableEndianess`
pub trait Readable {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized;
}

pub trait ReadableEndianness {
    fn read<T>(reader: &mut ByteStream) -> io::Result<Self>
    where
        T: ByteOrder,
        Self: Sized;
}

impl Readable for u8 {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        reader.reader.read_u8()
    }
}

impl Readable for i8 {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        reader.reader.read_i8()
    }
}

impl Readable for u16 {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        #[cfg(target_endian = "little")]
        {
            reader.reader.read_u16::<byteorder::LittleEndian>()
        }
        #[cfg(target_endian = "big")]
        {
            reader.reader.read_u16::<byteorder::BigEndian>()
        }
    }
}

impl ReadableEndianness for u16 {
    fn read<T>(reader: &mut ByteStream) -> io::Result<Self>
    where
        T: ByteOrder,
        Self: Sized,
    {
        reader.reader.read_u16::<T>()
    }
}

impl Readable for u32 {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        #[cfg(target_endian = "little")]
        {
            reader.reader.read_u32::<byteorder::LittleEndian>()
        }
        #[cfg(target_endian = "big")]
        {
            reader.reader.read_u32::<byteorder::BigEndian>()
        }
    }
}

impl ReadableEndianness for u32 {
    fn read<T>(reader: &mut ByteStream) -> io::Result<Self>
    where
        T: ByteOrder,
        Self: Sized,
    {
        reader.reader.read_u32::<T>()
    }
}

impl Readable for u64 {
    fn read(reader: &mut ByteStream) -> io::Result<Self>
    where
        Self: Sized,
    {
        #[cfg(target_endian = "little")]
        {
            reader.reader.read_u64::<byteorder::LittleEndian>()
        }
        #[cfg(target_endian = "big")]
        {
            reader.reader.read_u64::<byteorder::BigEndian>()
        }
    }
}

pub struct ByteStream {
    reader: BufReader<File>,
}

impl ByteStream {
    pub fn new(path: &Path) -> io::Result<Self> {
        let reader = BufReader::new(File::open(path)?);
        Ok(Self { reader })
    }

    pub fn get_byte_offset(&mut self) -> io::Result<u64> {
        self.reader.seek(SeekFrom::Current(0))
    }

    pub fn read_raw(&mut self, amount: usize) -> io::Result<Vec<u8>> {
        let mut buffer = vec![0u8; amount];
        self.reader.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    /// Reads raw bytes from sectors starting at `from` until `from + amount` without advancing the buffered reader's index.
    pub fn read_raw_sectors(&mut self, from: usize, amount: usize) -> io::Result<Vec<u8>> {
        let start = from * SECTOR_SIZE;
        let amt = amount * SECTOR_SIZE;
        self.read_raw_bytes(start, amt)
    }

    /// Reads raw bytes starting at `from` until `from + amount` without advancing the buffered reader's index.
    pub fn read_raw_bytes(&mut self, from: usize, amount: usize) -> io::Result<Vec<u8>> {
        let current = self.reader.seek(SeekFrom::Current(0))?;
        let mut buffer = vec![0u8; amount];
        let _ = self.reader.seek(SeekFrom::Start(from as u64))?;
        self.reader.read_exact(&mut buffer)?;
        let _ = self.reader.seek(SeekFrom::Start(current))?;
        Ok(buffer)
    }

    pub fn peek<T>(&mut self) -> io::Result<T>
    where
        T: Readable,
    {
        let current_index = self.reader.seek(SeekFrom::Current(0))?;
        let result = T::read(self)?;
        let _ = self.reader.seek(SeekFrom::Start(current_index))?;
        Ok(result)
    }

    pub fn read<T>(&mut self) -> io::Result<T>
    where
        T: Readable,
    {
        T::read(self)
    }

    pub fn read_le<T>(&mut self) -> io::Result<T>
    where
        T: ReadableEndianness,
    {
        T::read::<byteorder::LittleEndian>(self)
    }

    pub fn read_be<T>(&mut self) -> io::Result<T>
    where
        T: ReadableEndianness,
    {
        T::read::<byteorder::BigEndian>(self)
    }

    pub fn read_array<T, const S: usize>(&mut self) -> io::Result<[T; S]>
    where
        T: Readable + Copy,
    {
        let buffer = [T::read(self)?; S];
        Ok(buffer)
    }

    // Reads S bytes from the stream
    pub fn read_byte_array<const S: usize>(&mut self) -> io::Result<[u8; S]> {
        let mut buffer = [0u8; S];
        self.reader.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    pub fn jump_to_sector(&mut self, amount: u64) -> io::Result<()> {
        self.jump_to_byte(amount * SECTOR_SIZE as u64)
    }

    pub fn jump_to_byte(&mut self, amount: u64) -> io::Result<()> {
        self.reader.seek(SeekFrom::Start(amount))?;
        Ok(())
    }
}

pub fn interpret_bytes_as_utf16(name_bytes: &[u8]) -> Result<String, FromUtf16Error> {
    let num_bytes = name_bytes.len();
    let mut unicode_symbols: Vec<u16> = Vec::with_capacity(num_bytes / 2);
    for index in (0..num_bytes).step_by(2) {
        // Order of top and bottom here is reversed since the bytes are in little endian
        let first = name_bytes[index];
        let second = name_bytes[index + 1];
        unicode_symbols.push(bytes_to_u16(first, second));
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
