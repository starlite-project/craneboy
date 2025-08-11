use std::{iter, mem};

use serde::{Deserialize, Serialize};

use super::{MBC, StrResult, ram_banks, rom_banks};

#[derive(Debug, Serialize, Deserialize)]
pub struct MBC5 {
	rom: Vec<u8>,
	ram: Vec<u8>,
	rom_bank: usize,
	ram_bank: usize,
	ram_on: bool,
	ram_updated: bool,
	has_battery: bool,
	rom_banks: usize,
	ram_banks: usize,
}

impl MBC5 {
	pub fn new(data: Vec<u8>) -> StrResult<Self> {
		let subtype = data[0x147];
		let has_battery = matches!(subtype, 0x1B | 0x1E);
		let ram_banks = match subtype {
			0x1A | 0x1B | 0x1D | 0x1E => ram_banks(data[0x149]),
			_ => 0,
		};

		let ram_size = ram_banks * 0x2000;
		let rom_banks = rom_banks(data[0x148]);

		Ok(Self {
			rom: data,
			ram: iter::repeat_n(0, ram_size).collect(),
			rom_bank: 1,
			ram_bank: 0,
			ram_updated: false,
			ram_on: false,
			has_battery,
			rom_banks,
			ram_banks,
		})
	}
}

#[typetag::serde]
impl MBC for MBC5 {
	fn read_rom(&self, a: u16) -> u8 {
		let idx = if a < 0x4000 {
			a as usize
		} else {
			(self.rom_bank * 0x4000) | ((a as usize) & 0x3FFF)
		};

		self.rom.get(idx).copied().unwrap_or(0)
	}

	fn read_ram(&self, a: u16) -> u8 {
		if !self.ram_on {
			return 0;
		}

		self.ram[(self.ram_bank * 0x2000) | ((a as usize) & 0x1FFF)]
	}

	fn write_rom(&mut self, a: u16, v: u8) {
		match a {
			0x0000..=0x1FFF => self.ram_on = matches!(v & 0x0F, 0x0A),
			0x2000..=0x2FFF => {
				self.rom_bank = (self.rom_bank & 0x100) | ((v as usize) % self.rom_banks);
			}
			0x3000..=0x3FFF => {
				self.rom_bank =
					((self.rom_bank & 0x0FF) | (((v & 0x1) as usize) << 8)) % self.rom_banks;
			}
			0x4000..=0x5FFF => self.ram_bank = ((v & 0x0F) as usize) % self.ram_banks,
			0x6000..=0x7FFF => {}
			_ => panic!("could not write to {a:04X} (MBC5)"),
		}
	}

	fn write_ram(&mut self, a: u16, v: u8) {
		if !self.ram_on {
			return;
		}

		self.ram[(self.ram_bank * 0x2000) | ((a as usize) & 0x1FFF)] = v;
		self.ram_updated = true;
	}

	fn is_battery_backed(&self) -> bool {
		self.has_battery
	}

	fn load_ram(&mut self, ram_data: &[u8]) -> StrResult<()> {
		if ram_data.len() != self.ram.len() {
			return Err("loaded ram has incorrect length");
		}

		ram_data.clone_into(&mut self.ram);

		Ok(())
	}

	fn dump_ram(&self) -> Vec<u8> {
		self.ram.clone()
	}

	fn check_and_reset_ram_updated(&mut self) -> bool {
		mem::take(&mut self.ram_updated)
	}
}
