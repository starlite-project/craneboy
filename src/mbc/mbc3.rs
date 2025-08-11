use std::{io::prelude::*, iter, mem, time};

use serde::{Deserialize, Serialize};

use super::{MBC, StrResult, ram_banks};

#[derive(Debug, Serialize, Deserialize)]
pub struct MBC3 {
	rom: Vec<u8>,
	ram: Vec<u8>,
	rom_bank: usize,
	ram_bank: usize,
	ram_banks: usize,
	select_rtc: bool,
	ram_on: bool,
	ram_updated: bool,
	has_battery: bool,
	rtc_ram: [u8; 5],
	rtc_ram_latch: [u8; 5],
	rtc_zero: Option<u64>,
}

impl MBC3 {
	pub fn new(data: Vec<u8>) -> StrResult<Self> {
		let subtype = data[0x147];
		let has_battery = matches!(subtype, 0x0F | 0x10 | 0x13);
		let ram_banks = match subtype {
			0x10 | 0x12 | 0x13 => ram_banks(data[0x149]),
			_ => 0,
		};
		let ram_size = ram_banks * 0x2000;
		let rtc = match subtype {
			0x0F | 0x10 => Some(0),
			_ => None,
		};

		Ok(Self {
			rom: data,
			ram: iter::repeat_n(0, ram_size).collect(),
			rom_bank: 1,
			ram_bank: 0,
			ram_banks,
			select_rtc: false,
			ram_on: false,
			ram_updated: false,
			has_battery,
			rtc_ram: [0; 5],
			rtc_ram_latch: [0; 5],
			rtc_zero: rtc,
		})
	}

	fn latch_rtc_reg(&mut self) {
		self.calc_rtc_reg();
		self.rtc_ram_latch.clone_from_slice(&self.rtc_ram);
	}

	fn calc_rtc_reg(&mut self) {
		if matches!(self.rtc_ram[4] & 0x40, 0x40) {
			return;
		}

		let Some(tzero) = self
			.rtc_zero
			.map(|t| time::UNIX_EPOCH + time::Duration::from_secs(t))
		else {
			return;
		};

		if self.compute_difftime() == self.rtc_zero {
			return;
		}

		let difftime = time::SystemTime::now()
			.duration_since(tzero)
			.map(|n| n.as_secs())
			.unwrap_or(0);

		self.rtc_ram[0] = (difftime % 60) as u8;
		self.rtc_ram[1] = ((difftime / 60) % 60) as u8;
		self.rtc_ram[2] = ((difftime / 3600) % 24) as u8;
		let days = difftime / (3600 * 24);
		self.rtc_ram[3] = days as u8;
		self.rtc_ram[4] = (self.rtc_ram[4] & 0xFE) | (((days >> 8) & 0x01) as u8);
		if days >= 512 {
			self.rtc_ram[4] |= 0x80;
			self.calc_rtc_zero();
		}
	}

	fn compute_difftime(&self) -> Option<u64> {
		self.rtc_zero?;

		let mut difftime = match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
			Ok(t) => t.as_secs(),
			_ => panic!("system clock is set to a time before the unix epoc (1970-01-01)"),
		};

		difftime -= u64::from(self.rtc_ram[0]);
		difftime -= u64::from(self.rtc_ram[1]) * 60;
		difftime -= u64::from(self.rtc_ram[2]) * 3600;
		let days = ((u64::from(self.rtc_ram[4]) & 0x1) << 8) | u64::from(self.rtc_ram[3]);
		difftime -= days * 3600 * 24;
		Some(difftime)
	}

	fn calc_rtc_zero(&mut self) {
		self.rtc_zero = self.compute_difftime();
	}
}

#[typetag::serde]
impl MBC for MBC3 {
	fn read_rom(&self, a: u16) -> u8 {
		let idx = if a < 0x4000 {
			a as usize
		} else {
			(self.rom_bank * 0x4000) | ((a as usize) & 0x3FFF)
		};

		self.rom.get(idx).copied().unwrap_or(0xFF)
	}

	fn read_ram(&self, a: u16) -> u8 {
		if !self.ram_on {
			return 0xFF;
		}

		if !self.select_rtc && self.ram_bank < self.ram_banks {
			self.ram[(self.ram_bank * 0x2000) | ((a as usize) & 0x1FFF)]
		} else if self.select_rtc && self.ram_bank < 5 {
			self.rtc_ram_latch[self.ram_bank]
		} else {
			0xFF
		}
	}

	fn write_rom(&mut self, a: u16, v: u8) {
		match a {
			0x0000..=0x1FFF => self.ram_on = matches!(v & 0x0F, 0x0A),
			0x2000..=0x3FFF => {
				self.rom_bank = match v & 0x7F {
					0 => 1,
					n => n as usize,
				}
			}
			0x4000..=0x5FFF => {
				self.select_rtc = matches!(v & 0x8, 0x8);
				self.ram_bank = (v & 0x7) as usize;
			}
			0x6000..=0x7FFF => self.latch_rtc_reg(),
			_ => panic!("could not write to {a:04X} (MBC3)"),
		}
	}

	fn write_ram(&mut self, a: u16, v: u8) {
		if !self.ram_on {
			return;
		}

		if !self.select_rtc && self.ram_bank < self.ram_banks {
			self.ram[(self.ram_bank * 0x2000) | ((a as usize) & 0x1FFF)] = v;
			self.ram_updated = true;
		} else if self.select_rtc && self.ram_bank < 5 {
			self.calc_rtc_reg();
			let vmask = match self.ram_bank {
				0 | 1 => 0x3F,
				2 => 0x1F,
				4 => 0xC1,
				_ => 0xFF,
			};
			self.rtc_ram[self.ram_bank] = v & vmask;
			self.calc_rtc_zero();
			self.ram_updated = true;
		}
	}

	fn is_battery_backed(&self) -> bool {
		self.has_battery
	}

	fn load_ram(&mut self, ram_data: &[u8]) -> StrResult<()> {
		if ram_data.len() != self.ram.len() + 8 {
			return Err("loaded ram is too small");
		}

		let (int_bytes, rest) = ram_data.split_at(8);
		let rtc = u64::from_be_bytes(int_bytes.try_into().unwrap());
		if self.rtc_zero.is_some() {
			self.rtc_zero = Some(rtc);
		}

		rest.clone_into(&mut self.ram);
		Ok(())
	}

	fn dump_ram(&self) -> Vec<u8> {
		let rtc = self.rtc_zero.unwrap_or(0);

		let mut file = Vec::new();

		let mut ok = true;

		if ok {
			let rtc_bytes = rtc.to_be_bytes();
			ok = file.write_all(&rtc_bytes).is_ok();
		}

		if ok {
			_ = file.write_all(&self.ram);
		}

		file
	}

	fn check_and_reset_ram_updated(&mut self) -> bool {
		mem::take(&mut self.ram_updated)
	}
}
