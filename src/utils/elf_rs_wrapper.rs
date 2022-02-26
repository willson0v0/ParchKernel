use elf_rs::*;
use elf_rs::Elf;


use super::ErrorNum;

pub type ELFFile<'a> = Elf64<'a>;

pub fn read_elf(bytes: &[u8]) -> Result<ELFFile, ErrorNum> {
    let elf = Elf::from_bytes(&bytes);
    if elf.is_err() {
        return Err(ErrorNum::ENOEXEC);
    }
    let res: ELFFile = match elf.unwrap() {
        Elf::Elf64(elf) => elf,
        _ => return Err(ErrorNum::ENOEXEC),
    };

    Ok(res)
}