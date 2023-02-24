# MBR Parser
Small command line utility for printing the partition tables of a master boot record.

## Example
```
+----------+---------------------+-------------------+---------------+-------------------------+
| Bootable | LBA Starting Sector | LBA Ending Sector | Total Sectors | Partition Type          |
+----------+---------------------+-------------------+---------------+-------------------------+
| No       | 128                 | 1566848           | 1566720       | 0x07 :: HPFS/NTFS/exFAT |
+----------+---------------------+-------------------+---------------+-------------------------+
| No       | 1566848             | 1599616           | 32768         | 0x07 :: HPFS/NTFS/exFAT |
+----------+---------------------+-------------------+---------------+-------------------------+
| No       | 1599616             | 1632384           | 32768         | 0x07 :: HPFS/NTFS/exFAT |
+----------+---------------------+-------------------+---------------+-------------------------+
| No       | 1632384             | 2093184           | 460800        | 0x05 :: Extended        |
+----------+---------------------+-------------------+---------------+-------------------------+
| No       | 1632512             | 1665280           | 32768         | 0x07 :: HPFS/NTFS/exFAT |
+----------+---------------------+-------------------+---------------+-------------------------+
| No       | 1665408             | 1698176           | 32768         | 0x07 :: HPFS/NTFS/exFAT |
+----------+---------------------+-------------------+---------------+-------------------------+
| No       | 1698304             | 2089472           | 391168        | 0x07 :: HPFS/NTFS/exFAT |
+----------+---------------------+-------------------+---------------+-------------------------+
```

## Usage 
```
Usage: mbr_parser [OPTIONS] <IMAGE_PATH>

Arguments:
  <IMAGE_PATH>  

Options:
      --show-chs  
  -h, --help      Print help
```

## Install
Prerequisite: [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)

Once cargo is installed, clone this repo and install using the following command:  
`cargo install --path <mbr_parser repo>`  
Where `<mbr_parser repo>` is the path to the cloned repo.

Alternatively build the program with Cargo:
```
cd mbr_parser
cargo build --release
```
 and run it from `mbr_parser/target/release/mbr_parser`