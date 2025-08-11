mod mbc0;
mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;

use std::{
	fs::{self, File},
	io::{self, prelude::*},
	path::PathBuf,
};

use serde::{Deserialize, Serialize};

use super::StrResult;

#[derive(Serialize, Deserialize)]
pub struct FileBackedMBC {
	ram_path: PathBuf,
	mbc: Box<dyn MBC>,
}

impl FileBackedMBC {
	pub fn new(rom_path: PathBuf, skip_checksum: bool) -> StrResult<Self> {
		let mut data = Vec::new();
		File::open(&rom_path)
			.and_then(|mut f| f.read_to_end(&mut data))
			.map_err(|_| "could not read rom")?;

		let mut mbc = get_mbc(data, skip_checksum)?;

		let ram_path = rom_path.with_extension("gbsave");

		if mbc.is_battery_backed() {
			match File::open(&ram_path) {
				Ok(mut file) => {
					let mut ram_data = Vec::new();
					match file.read_to_end(&mut ram_data) {
						Ok(..) => mbc.load_ram(&ram_data)?,
						Err(..) => return Err("error while reading existing save file"),
					}
				}
				Err(e) if matches!(e.kind(), io::ErrorKind::NotFound) => {}
				Err(..) => return Err("error loading existing save file"),
			}
		}

		Ok(Self { ram_path, mbc })
	}
}

impl Drop for FileBackedMBC {
	fn drop(&mut self) {
		if self.mbc.is_battery_backed() {
			let Ok(mut file) = fs::File::create(&self.ram_path) else {
				return;
			};

			let _ = file.write_all(&self.mbc.dump_ram());
		}
	}
}

#[typetag::serde]
impl MBC for FileBackedMBC {
	fn read_rom(&self, a: u16) -> u8 {
		self.mbc.read_rom(a)
	}

	fn read_ram(&self, a: u16) -> u8 {
		self.mbc.read_ram(a)
	}

	fn write_rom(&mut self, a: u16, v: u8) {
		self.mbc.write_rom(a, v);
	}

	fn write_ram(&mut self, a: u16, v: u8) {
		self.mbc.write_ram(a, v);
	}

	fn is_battery_backed(&self) -> bool {
		self.mbc.is_battery_backed()
	}

	fn load_ram(&mut self, ram_data: &[u8]) -> StrResult<()> {
		self.mbc.load_ram(ram_data)
	}

	fn dump_ram(&self) -> Vec<u8> {
		self.mbc.dump_ram()
	}

	fn check_and_reset_ram_updated(&mut self) -> bool {
		self.mbc.check_and_reset_ram_updated()
	}
}

#[typetag::serde(tag = "type")]
pub trait MBC: Send {
	fn read_rom(&self, a: u16) -> u8;

	fn read_ram(&self, a: u16) -> u8;

	fn write_rom(&mut self, a: u16, v: u8);

	fn write_ram(&mut self, a: u16, v: u8);

	fn check_and_reset_ram_updated(&mut self) -> bool;

	fn is_battery_backed(&self) -> bool;

	fn load_ram(&mut self, ram_data: &[u8]) -> StrResult<()>;

	fn dump_ram(&self) -> Vec<u8>;

	fn rom_name(&self) -> String {
		const TITLE_START: u16 = 0x134;
		const CGB_FLAG: u16 = 0x143;

		let title_size = match self.read_rom(CGB_FLAG) & 0x80 {
			0x80 => 11,
			_ => 16,
		};

		let mut result = String::with_capacity(title_size as usize);

		for i in 0..title_size {
			match self.read_rom(TITLE_START + i) {
				0 => break,
				v => result.push(v as char),
			}
		}

		result
	}
}

pub fn get_mbc(data: Vec<u8>, skip_checksum: bool) -> StrResult<Box<dyn MBC + 'static>> {
	if data.len() < 0x150 {
		return Err("rom size too small");
	}

	if !skip_checksum {
		check_checksum(&data)?;
	}

	match data[0x147] {
		0x00 => self::mbc0::MBC0::new(data).map(|v| Box::new(v) as Box<dyn MBC>),
		0x01..=0x03 => self::mbc1::MBC1::new(data).map(|v| Box::new(v) as Box<dyn MBC>),
		0x05..=0x06 => self::mbc2::MBC2::new(data).map(|v| Box::new(v) as Box<dyn MBC>),
		0x0F..=0x13 => self::mbc3::MBC3::new(data).map(|v| Box::new(v) as Box<dyn MBC>),
		0x19..=0x1E => self::mbc5::MBC5::new(data).map(|v| Box::new(v) as Box<dyn MBC>),
		_ => Err("unsupported mbc type"),
	}
}

const fn ram_banks(v: u8) -> usize {
	match v {
		1 | 2 => 1,
		3 => 4,
		4 => 16,
		5 => 8,
		_ => 0,
	}
}

const fn rom_banks(v: u8) -> usize {
	if v <= 8 { 2 << v } else { 0 }
}

fn check_checksum(data: &[u8]) -> StrResult<()> {
	let mut value = 0u8;
	for i in 0x134..0x14D {
		value = value.wrapping_sub(data[i]).wrapping_sub(1);
	}

	if data[0x14D] == value {
		Ok(())
	} else {
		Err("cartridge checksum is invalid")
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn checksum_zero() {
		let mut data = [0; 0x150];
		data[0x14D] = -(0x14Di32 - 0x134i32) as u8;

		super::check_checksum(&data).unwrap();
	}

	#[test]
	fn checksum_ones() {
		let mut data = [1; 0x150];
		data[0x14D] = (-(0x14Di32 - 0x134i32) * 2) as u8;

		super::check_checksum(&data).unwrap();
	}
}
