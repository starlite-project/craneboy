use std::{iter, mem};

use serde::{Deserialize, Serialize};

use super::{MBC, StrResult, ram_banks, rom_banks};

#[derive(Debug, Serialize, Deserialize)]
pub struct MBC1 {
	rom: Vec<u8>,
	ram: Vec<u8>,
	ram_on: bool,
	ram_updated: bool,
	banking_mode: u8,
	rom_bank: usize,
	ram_bank: usize,
	has_battery: bool,
	rom_banks: usize,
	ram_banks: usize,
}

impl MBC1 {
	pub fn new(data: Vec<u8>) -> StrResult<Self> {
		let (has_battery, ram_banks) = match data[0x147] {
			0x02 => (false, ram_banks(data[0x149])),
			0x03 => (true, ram_banks(data[0x149])),
			_ => (false, 0),
		};

		let rom_banks = rom_banks(data[0x148]);
		let ram_size = ram_banks * 0x2000;

		Ok(Self {
			rom: data,
			ram: iter::repeat_n(0u8, ram_size).collect(),
			ram_on: false,
			banking_mode: 0,
			rom_bank: 1,
			ram_bank: 0,
			ram_updated: false,
			has_battery,
			rom_banks,
			ram_banks,
		})
	}
}

#[typetag::serde]
impl MBC for MBC1 {
	fn read_rom(&self, a: u16) -> u8 {
		let bank = if a < 0x4000 {
			if matches!(self.banking_mode, 0) {
				0
			} else {
				self.rom_bank & 0xE0
			}
		} else {
			self.rom_bank
		};

		let idx = (bank * 0x4000) | ((a as usize) & 0x3FFF);
		self.rom.get(idx).copied().unwrap_or(0xFF)
	}

	fn read_ram(&self, a: u16) -> u8 {
		if !self.ram_on {
			return 0xFF;
		}

		let ram_bank = if matches!(self.banking_mode, 1) {
			self.ram_bank
		} else {
			0
		};

		self.ram[(ram_bank * 0x2000) | ((a & 0x1FFF) as usize)]
	}

	fn write_rom(&mut self, a: u16, v: u8) {
		match a {
			0x0000..=0x1FFF => {
				self.ram_on = matches!(v & 0xF, 0xA);
			}
			0x2000..=0x3FFF => {
				let lower_bits = match (v as usize) & 0x1F {
					0 => 1,
					n => n,
				};
				self.rom_bank = ((self.rom_bank & 0x60) | lower_bits) % self.rom_banks;
			}
			0x4000..=0x5FFF => {
				if self.rom_banks > 0x20 {
					let upper_bits = (v as usize & 0x03) % (self.rom_banks >> 5);
					self.rom_bank = self.rom_bank & 0x1F | (upper_bits << 5);
				}

				if self.ram_banks > 1 {
					self.ram_bank = (v as usize) & 0x03;
				}
			}
			0x6000..=0x7FFF => {
				self.banking_mode = v & 0x01;
			}
			_ => panic!("could not write to {a:04X} (MBC1)"),
		}
	}

	fn write_ram(&mut self, a: u16, v: u8) {
		if !self.ram_on {
			return;
		}

		let ram_bank = if matches!(self.banking_mode, 1) {
			self.ram_bank
		} else {
			0
		};

		let address = (ram_bank * 0x2000) | ((a & 0x1FFF) as usize);
		if address < self.ram.len() {
			self.ram[address] = v;
			self.ram_updated = true;
		}
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
