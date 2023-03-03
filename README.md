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
| 34                  | 32767             | 32734         | 16         | Microsoft reserved   |
+---------------------+-------------------+---------------+------------+----------------------+
| 32768               | 2093055           | 2060288       | 1006       | Microsoft basic data |
+---------------------+-------------------+---------------+------------+----------------------+

loucas:~$ parttable mbr_test.dd --extract-mft
 - $MFTMirr
 - $LogFile
 - $Volume
 - $AttrDef
 - .
 - $Bitmap
 - $Boot
 - $BadClus
 - $Secure
 - $UpCase
 - $Extend
 - $Quota
 - $ObjId
 - $Reparse
 - $RmMetadata
 - $Repair
 - $Deleted
 - $TxfLog
 - $Txf
 - $Tops
 - $TxfLog.blf
 - $TxfLogContainer00000000000000000001
 - $TxfLogContainer00000000000000000002
 - System Volume Information
 - WPSettings.dat
 - tracking.log
 - $RECYCLE.BIN
 - S-1-5-21-4215114664-1519948314-2148250071-1001
 - desktop.ini
 - IndexerVolumeGuid
 - MSIf6798.tmp
```

## Usage 
```
Usage: parttable [OPTIONS] <IMAGE_PATH>

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

## Install
Install the [debian package](https://github.com/kregerl/parttable/releases/latest) or compile using cargo.

Make sure you have [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) installed, then clone this repo and install using the following command:  
`cargo install --path <parttable repo>`  
Where `<parttable repo>` is the path to the cloned repo.  
*NOTE: This will only install the program for your user in ~/.cargo/bin*

Alternatively build or install the program with Cargo:
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
