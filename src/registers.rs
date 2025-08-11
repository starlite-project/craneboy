use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use super::gb_mode::GbMode;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Registers {
	pub a: u8,
	f: CpuFlags,
	pub b: u8,
	pub c: u8,
	pub d: u8,
	pub e: u8,
	pub h: u8,
	pub l: u8,
	pub pc: u16,
	pub sp: u16,
}

impl Registers {
	pub fn new(mode: GbMode) -> Self {
		match mode {
			GbMode::Classic => Self {
				a: 0x01,
				f: CpuFlags::C | CpuFlags::H | CpuFlags::Z,
				b: 0x00,
				c: 0x13,
				d: 0x00,
				e: 0xD8,
				h: 0x01,
				l: 0x4D,
				pc: 0x0100,
				sp: 0xFFFE,
			},
			GbMode::ColorAsClassic => Self {
				a: 0x11,
				f: CpuFlags::Z,
				b: 0x00,
				c: 0x00,
				d: 0x00,
				e: 0x08,
				h: 0x00,
				l: 0x7C,
				pc: 0x0100,
				sp: 0xFFFE,
			},
			GbMode::Color => Self {
				a: 0x11,
				f: CpuFlags::Z,
				b: 0x00,
				c: 0x00,
				d: 0xFF,
				e: 0x56,
				h: 0x00,
				l: 0x0D,
				pc: 0x0100,
				sp: 0xFFFE,
			},
		}
	}

	pub const fn af(self) -> u16 {
		((self.a as u16) << 8) | ((self.f.bits() & 0xF0) as u16)
	}

	pub const fn bc(self) -> u16 {
		((self.b as u16) << 8) | (self.c as u16)
	}

	pub const fn de(self) -> u16 {
		((self.d as u16) << 8) | (self.e as u16)
	}

	pub const fn hl(self) -> u16 {
		((self.h as u16) << 8) | (self.l as u16)
	}

	pub const fn hld(&mut self) -> u16 {
		let res = self.hl();
		self.set_hl(res - 1);
		res
	}

	pub const fn hli(&mut self) -> u16 {
		let res = self.hl();
		self.set_hl(res + 1);
		res
	}

	pub const fn set_af(&mut self, value: u16) {
		self.a = (value >> 8) as u8;
		self.f = CpuFlags::from_bits_truncate((value & 0x00F0) as u8);
	}

	pub const fn set_bc(&mut self, value: u16) {
		self.b = (value >> 8) as u8;
		self.c = (value & 0x00FF) as u8;
	}

	pub const fn set_de(&mut self, value: u16) {
		self.d = (value >> 8) as u8;
		self.e = (value & 0x00FF) as u8;
	}

	pub const fn set_hl(&mut self, value: u16) {
		self.h = (value >> 8) as u8;
		self.l = (value & 0x00FF) as u8;
	}

	pub fn flag(&mut self, flags: CpuFlags, set: bool) {
		if set {
			self.f |= flags;
		} else {
			self.f &= !flags;
		}

		self.f &= CpuFlags::all();
	}

	pub const fn get_flag(self, flag: CpuFlags) -> bool {
		self.f.contains(flag)
	}

	#[cfg(test)]
	fn set_f(&mut self, flags: u8) {
		self.f = CpuFlags::from_bits_retain(flags) & CpuFlags::all();
	}
}

bitflags! {
	#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
	pub struct CpuFlags: u8 {
		const C = 0b0001_0000;
		const H = 0b0010_0000;
		const N = 0b0100_0000;
		const Z = 0b1000_0000;

		const _ = !0;
	}
}

#[cfg(test)]
mod tests {
	use super::{CpuFlags, GbMode, Registers};

	#[test]
	fn wide_registers() {
		let mut reg = Registers::new(GbMode::Classic);
		reg.a = 0x12;
		reg.set_f(0x23);
		reg.b = 0x34;
		reg.c = 0x45;
		reg.d = 0x56;
		reg.e = 0x67;
		reg.h = 0x78;
		reg.l = 0x89;

		assert_eq!(reg.af(), 0x1220);
		assert_eq!(reg.bc(), 0x3445);
		assert_eq!(reg.de(), 0x5667);
		assert_eq!(reg.hl(), 0x7889);

		reg.set_af(0x1111);
		reg.set_bc(0x1111);
		reg.set_de(0x1111);
		reg.set_hl(0x1111);
		assert_eq!(reg.af(), 0x1110);
		assert_eq!(reg.bc(), 0x1111);
		assert_eq!(reg.de(), 0x1111);
		assert_eq!(reg.hl(), 0x1111);
	}

	#[test]
	fn flags() {
		let mut reg = Registers::new(GbMode::Classic);
		let flags = [CpuFlags::C, CpuFlags::H, CpuFlags::N, CpuFlags::Z];

		assert_eq!(reg.f.bits() & 0x0F, 0);

		reg.set_f(0x00);

		for mask in flags {
			assert!(!reg.get_flag(mask));
			reg.flag(mask, true);
			assert!(reg.get_flag(mask));
			reg.flag(mask, false);
			assert!(!reg.get_flag(mask));
		}
	}

	#[test]
	fn hl_special() {
		let mut reg = Registers::new(GbMode::Classic);
		reg.set_hl(0x1234);
		assert_eq!(reg.hl(), 0x1234);
		assert_eq!(reg.hld(), 0x1234);
		assert_eq!(reg.hld(), 0x1233);
		assert_eq!(reg.hld(), 0x1232);
		assert_eq!(reg.hli(), 0x1231);
		assert_eq!(reg.hli(), 0x1232);
		assert_eq!(reg.hli(), 0x1233);
		assert_eq!(reg.hl(), 0x1234);
	}
}
