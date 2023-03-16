# Parttable
Small command line utility for printing the partition tables. Currently supported partition tables: 
- Guid Partition Table (GPT)
- Master Boot Record (MBR)
- Apple Partition Map (APM)

The Master File Table (MFT) can also be extracted from an NTFS partition when the partitioning scheme is GPT or MBR.  

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
Header guid: 88981628-4F29-4224-A409-E247B756F0D4

+---------------------+-------------------+---------------+------------+----------------------+
| LBA Starting Sector | LBA Ending Sector | Total Sectors | Size (MB)  | Partition Type       |
+---------------------+-------------------+---------------+------------+----------------------+
| 34                  | 32767             | 32734         | 16         | Microsoft reserved   |
+---------------------+-------------------+---------------+------------+----------------------+
| 32768               | 2093055           | 2060288       | 1006       | Microsoft basic data |
+---------------------+-------------------+---------------+------------+----------------------+

loucas:~$ parttable apm_1_partition.dd
+--------------+------------+-----------------+----------------+---------------------+
| Starting LBA | Ending LBA | Size in Sectors | Partition Name | Partition Type      |
+--------------+------------+-----------------+----------------+---------------------+
| 1            | 63         | 63              | Apple          | Apple_partition_map |
+--------------+------------+-----------------+----------------+---------------------+
| 64           | 2097135    | 2097072         |                | Apple_HFS           |
+--------------+------------+-----------------+----------------+---------------------+
| 2097136      | 2097151    | 16              |                | Apple_Free          |
+--------------+------------+-----------------+----------------+---------------------+

loucas:~$ parttable mbr_test.dd --extract-mft
 +-----------+----------------------------+----------------------------+----------------------------+----------------------------+      
| File Name | $FN Modified               | $FN MFT Modified           | $FN Created                | $FN Read                   |      
+-----------+----------------------------+----------------------------+----------------------------+----------------------------+      
| $MFT      | 2023-03-02 17:48:33 -05:00 | 2023-03-02 17:48:33 -05:00 | 2023-03-02 17:48:33 -05:00 | 2023-03-02 17:48:33 -05:00 |      
+-----------+----------------------------+----------------------------+----------------------------+----------------------------+      
+-----------------+----------------------------+----------------------------+----------------------------+----------------------------+
| $SI Byte Offset | $SI Modified               | $SI MFT Modified           | $SI Created                | $SI Read                   |
+-----------------+----------------------------+----------------------------+----------------------------+----------------------------+
| 356929616       | 2023-03-02 17:48:33 -05:00 | 2023-03-02 17:48:33 -05:00 | 2023-03-02 17:48:33 -05:00 | 2023-03-02 17:48:33 -05:00 |
+-----------------+----------------------------+----------------------------+----------------------------+----------------------------+
...
```

## Usage 
```
Usage: parttable.exe [OPTIONS] <IMAGE_PATH> [COMMAND]

Commands:
  timestomp  Timestomp `file_name` with the `timestamp`
  help       Print this message or the help of the given subcommand(s)

Arguments:
  <IMAGE_PATH>  

Options:
      --show-chs     
      --extract-mft
  -h, --help         Print help
```
### Extract MFT
The option `extract-mft` can be used to read the file names from NTFS partitions regardless of partitioning scheme.  
The parser will only extract $STANDARD_INFORMATION and $FILE_NAME attributes for most MFT file records since that is all thats needed to attempt [timestomping](https://attack.mitre.org/techniques/T1070/006/) for a given file.
The $DATA attribute is only extracted for the $MFT file entry so the MFT's size can be known.

### Timestomping
To timestomp a file in the MFT, the timestomp subcommand can be used as shown below:
```
Timestomp `file_name` with the `timestamp`

Usage: parttable <IMAGE_PATH> timestomp <FILE_NAME> <TIMESTAMP>

Arguments:
  <FILE_NAME>  Name of the file entry in the MFT
  <TIMESTAMP>  Unix epoch timestamp to timestomp with

Options:
  -h, --help  Print help
```
The file name must exist in the MFT and the timestamp is expected to be given in [unix epoch](https://www.epochconverter.com/)


## Install
Install the [debian package](https://github.com/kregerl/parttable/releases/latest) or compile using cargo.

Alternatively build or install the program with Cargo:  
Make sure you have [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) installed, then clone this repo and install using the following command:  
`cargo install --path <parttable repo>`  
Where `<parttable repo>` is the path to the cloned repo.  
*NOTE: This will only install the program for your user in ~/.cargo/bin*

```
git clone https://github.com/kregerl/parttable.git
cd parttable
cargo install --path .
parttable --help
```

```
git clone https://github.com/kregerl/parttable.git
cd parttable
cargo build --release
./target/release/parttable
```
