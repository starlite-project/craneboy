use serde::{Deserialize, Serialize};

use super::{MBC, StrResult};

#[derive(Debug, Serialize, Deserialize)]
#[repr(transparent)]
pub struct MBC0 {
	rom: Vec<u8>,
}

impl MBC0 {
	pub const fn new(data: Vec<u8>) -> StrResult<Self> {
		Ok(Self { rom: data })
	}
}

#[typetag::serde]
impl MBC for MBC0 {
	fn read_rom(&self, a: u16) -> u8 {
		self.rom[a as usize]
	}

	fn read_ram(&self, _: u16) -> u8 {
		0
	}

	fn write_rom(&mut self, _: u16, _: u8) {}

	fn write_ram(&mut self, _: u16, _: u8) {}

	fn is_battery_backed(&self) -> bool {
		false
	}

	fn load_ram(&mut self, _: &[u8]) -> StrResult<()> {
		Ok(())
	}

	fn dump_ram(&self) -> Vec<u8> {
		Vec::new()
	}

	fn check_and_reset_ram_updated(&mut self) -> bool {
		false
	}
}
