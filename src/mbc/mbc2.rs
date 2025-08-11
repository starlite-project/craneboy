use std::mem;

use serde::{Deserialize, Serialize};

use super::{MBC, StrResult, rom_banks};

#[derive(Debug, Serialize, Deserialize)]
pub struct MBC2 {
	rom: Vec<u8>,
	ram: Vec<u8>,
	ram_on: bool,
	ram_updated: bool,
	rom_bank: usize,
	has_battery: bool,
	rom_banks: usize,
}

impl MBC2 {
	pub fn new(data: Vec<u8>) -> StrResult<Self> {
		let has_battery = matches!(data[0x147], 0x06);
		let rom_banks = rom_banks(data[0x148]);

		Ok(Self {
			rom: data,
			ram: vec![0; 512],
			ram_on: false,
			ram_updated: false,
			rom_bank: 1,
			has_battery,
			rom_banks,
		})
	}
}

#[typetag::serde]
impl MBC for MBC2 {
	fn read_rom(&self, a: u16) -> u8 {
		let bank = if a < 0x4000 { 0 } else { self.rom_bank };
		let idx = (bank * 0x4000) | ((a as usize) & 0x3FFF);
		self.rom.get(idx).copied().unwrap_or(0xFF)
	}

	fn read_ram(&self, a: u16) -> u8 {
		if !self.ram_on {
			return 0xFF;
		}

		self.ram[(a as usize) & 0x1FF] | 0xF0
	}

	fn write_rom(&mut self, a: u16, v: u8) {
		if let 0x0000..=0x3FFF = a {
			if matches!(a & 0x100, 0) {
				self.ram_on = matches!(v & 0xF, 0xA);
			} else {
				self.rom_bank = match (v as usize) & 0x0F {
					0 => 1,
					n => n,
				} % self.rom_banks;
			}
		}
	}

	fn write_ram(&mut self, a: u16, v: u8) {
		if !self.ram_on {
			return;
		}

		self.ram[(a as usize) & 0x1FF] = v | 0xF0;
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
