# Parttable
Small command line utility for printing the partition tables of a master boot record.

## Example
```
loucas:~$ parttable mbr_test.dd
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

loucas:~$ parttable gpt_test.dd
+---------------------+-------------------+---------------+------------+----------------------+
| LBA Starting Sector | LBA Ending Sector | Total Sectors | Size (MB)  | Partition Type       |
+---------------------+-------------------+---------------+------------+----------------------+
| 34                  | 32767             | 32734         | 16759808   | Microsoft reserved   |
+---------------------+-------------------+---------------+------------+----------------------+
| 32768               | 2093055           | 2060288       | 1054867456 | Microsoft basic data |
+---------------------+-------------------+---------------+------------+----------------------+
```

## Usage 
```
Usage: parttable [OPTIONS] <IMAGE_PATH>

Arguments:
  <IMAGE_PATH>  

Options:
      --show-chs  
  -h, --help      Print help
```

## Install
Install the [debian package](https://github.com/kregerl/parttable/releases/latest) or compile using cargo.

Make sure you have [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) installed, then clone this repo and install using the following command:  
`cargo install --path <mbr_parser repo>`  
Where `<parttable repo>` is the path to the cloned repo.  
*NOTE: This will only install the program in ~/.cargo/bin for your user*

Alternatively build the program with Cargo:
```
git clone https://github.com/kregerl/parttable.git
cd parttable
cargo build --release
./target/release/parttable
```
