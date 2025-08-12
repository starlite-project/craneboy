use std::mem;

use serde::{Deserialize, Serialize};

use super::{
	StrResult,
	gb_mode::{GbMode, GbSpeed},
	gpu::GPU,
	keypad::Keypad,
	mbc,
	serial::{Serial, SerialCallback},
	sound::Sound,
	timer::Timer,
};

const WRAM_SIZE: usize = 0x8000;
const ZRAM_SIZE: usize = 0x7F;

#[derive(Serialize, Deserialize)]
pub struct MMU {
	#[serde(with = "serde_arrays")]
	wram: [u8; WRAM_SIZE],
	#[serde(with = "serde_arrays")]
	zram: [u8; ZRAM_SIZE],
	hdma: [u8; 4],
	pub inte: u8,
	pub intf: u8,
	pub serial: Serial,
	pub timer: Timer,
	pub keypad: Keypad,
	pub gpu: GPU,
	#[serde(skip)]
	pub sound: Option<Sound>,
	hdma_status: DMAType,
	hdma_src: u16,
	hdma_dst: u16,
	hdma_len: u8,
	wram_bank: usize,
	pub mbc: Box<dyn mbc::MBC + 'static>,
	pub gbmode: GbMode,
	gbspeed: GbSpeed,
	speed_switch_req: bool,
	undocumented_cgb_regs: [u8; 3], // 0xFF72, 0xFF73, 0xFF75
}

impl MMU {
	pub fn classic(
		cart: Box<dyn mbc::MBC + 'static>,
		serial_callback: Option<Box<dyn SerialCallback>>,
	) -> StrResult<Self> {
		let serial = match serial_callback {
			Some(cb) => Serial::with_callback(cb),
			None => Serial::new(),
		};

		let mut res = Self {
			wram: [0; WRAM_SIZE],
			zram: [0; ZRAM_SIZE],
			hdma: [0; 4],
			wram_bank: 1,
			inte: 0,
			intf: 0,
			serial,
			timer: Timer::new(),
			keypad: Keypad::new(),
			gpu: GPU::classic(),
			sound: None,
			mbc: cart,
			gbmode: GbMode::Classic,
			gbspeed: GbSpeed::Single,
			speed_switch_req: false,
			hdma_src: 0,
			hdma_dst: 0,
			hdma_status: DMAType::NoDMA,
			hdma_len: 0xFF,
			undocumented_cgb_regs: [0; 3],
		};

		fill_random(&mut res.wram, 42);

		if matches!(res.rb(0x0143), 0xC0) {
			return Err("this game does not work in classic mode");
		}

		res.set_initial();
		Ok(res)
	}

	pub fn cgb(
		cart: Box<dyn mbc::MBC + 'static>,
		serial_callback: Option<Box<dyn SerialCallback>>,
	) -> StrResult<Self> {
		let serial = match serial_callback {
			Some(cb) => Serial::with_callback(cb),
			None => Serial::new(),
		};

		let mut res = Self {
			wram: [0; WRAM_SIZE],
			zram: [0; ZRAM_SIZE],
			wram_bank: 1,
			hdma: [0; 4],
			inte: 0,
			intf: 0,
			serial,
			timer: Timer::new(),
			keypad: Keypad::new(),
			gpu: GPU::cgb(),
			sound: None,
			mbc: cart,
			gbmode: GbMode::Color,
			gbspeed: GbSpeed::Single,
			speed_switch_req: false,
			hdma_src: 0,
			hdma_dst: 0,
			hdma_status: DMAType::NoDMA,
			hdma_len: 0xFF,
			undocumented_cgb_regs: [0; 3],
		};

		fill_random(&mut res.wram, 42);
		res.determine_mode();
		res.set_initial();

		Ok(res)
	}

	fn determine_mode(&mut self) {
		let mode = match self.rb(0x0143) & 0x80 {
			0x80 => GbMode::Color,
			_ => GbMode::ColorAsClassic,
		};

		self.gbmode = mode;
		self.gpu.gbmode = mode;
	}

	fn set_initial(&mut self) {
		self.wb(0xFF05, 0);
		self.wb(0xFF06, 0);
		self.wb(0xFF07, 0);
		self.wb(0xFF10, 0x80);
		self.wb(0xFF11, 0xBF);
		self.wb(0xFF12, 0xF3);
		self.wb(0xFF14, 0xBF);
		self.wb(0xFF16, 0x3F);
		self.wb(0xFF16, 0x3F);
		self.wb(0xFF17, 0);
		self.wb(0xFF19, 0xBF);
		self.wb(0xFF1A, 0x7F);
		self.wb(0xFF1B, 0xFF);
		self.wb(0xFF1C, 0x9F);
		self.wb(0xFF1E, 0xFF);
		self.wb(0xFF20, 0xFF);
		self.wb(0xFF21, 0);
		self.wb(0xFF22, 0);
		self.wb(0xFF23, 0xBF);
		self.wb(0xFF24, 0x77);
		self.wb(0xFF25, 0xF3);
		self.wb(0xFF26, 0xF1);
		self.wb(0xFF40, 0x91);
		self.wb(0xFF42, 0);
		self.wb(0xFF43, 0);
		self.wb(0xFF45, 0);
		self.wb(0xFF47, 0xFC);
		self.wb(0xFF48, 0xFF);
		self.wb(0xFF49, 0xFF);
		self.wb(0xFF4A, 0);
		self.wb(0xFF4B, 0);
	}

	#[expect(unreachable_patterns, reason = "false positive")]
	pub fn rb(&mut self, address: u16) -> u8 {
		match address {
			0x0000..=0x7FFF => self.mbc.read_rom(address),
			0x8000..=0x9FFF | 0xFE00..=0xFE9F | 0xFF40..=0xFF4F | 0xFF68..=0xFF6B => {
				self.gpu.rb(address)
			}
			0xA000..=0xBFFF => self.mbc.read_ram(address),
			0xC000..=0xCFFF | 0xE000..=0xEFFF => self.wram[address as usize & 0x0FFF],
			0xD000..=0xDFFF | 0xF000..=0xFDFF => {
				self.wram[(self.wram_bank * 0x1000) | address as usize & 0x0FFF]
			}
			0xFF00 => self.keypad.rb(),
			0xFF01..=0xFF02 => self.serial.rb(address),
			0xFF04..=0xFF07 => self.timer.rb(address),
			0xFF0F => self.intf | 0b1110_0000,
			0xFF10..=0xFF3F => self.sound.as_mut().map_or(0xFF, |s| s.rb(address)),
			0xFF4D | 0xFF4F | 0xFF51..=0xFF55 | 0xFF6C | 0xFF70
				if !matches!(self.gbmode, GbMode::Color) =>
			{
				0xFF
			}
			0xFF72..=0xFF73 | 0xFF75..=0xFF77 if matches!(self.gbmode, GbMode::Classic) => 0xFF,
			0xFF4D => {
				0b0111_1110
					| (if matches!(self.gbspeed, GbSpeed::Double) {
						0x80
					} else {
						0
					}) | u8::from(self.speed_switch_req)
			}
			0xFF51..=0xFF55 => self.hdma_read(address),
			0xFF70 => self.wram_bank as u8,
			0xFF72..=0xFF73 => self.undocumented_cgb_regs[address as usize - 0xFF72],
			0xFF75 => self.undocumented_cgb_regs[2] | 0b1000_1111,
			0xFF76..=0xFF77 => 0x00,
			0xFF80..=0xFFFE => self.zram[address as usize & 0x007F],
			0xFFFF => self.inte,
			_ => 0xFF,
		}
	}

	pub fn rw(&mut self, address: u16) -> u16 {
		u16::from(self.rb(address)) | (u16::from(self.rb(address + 1)) << 8)
	}

	#[expect(unreachable_patterns, reason = "false positive")]
	pub fn wb(&mut self, address: u16, value: u8) {
		match address {
			0x0000..=0x7FFF => self.mbc.write_rom(address, value),
			0x8000..=0x9FFF | 0xFE00..=0xFE9F | 0xFF40..=0xFF4F | 0xFF68..=0xFF6B => {
				self.gpu.wb(address, value);
			}
			0xA000..=0xBFFF => self.mbc.write_ram(address, value),
			0xC000..=0xCFFF | 0xE000..=0xEFFF => self.wram[address as usize & 0x0FFF] = value,
			0xD000..=0xDFFF | 0xF000..=0xFDFF => {
				self.wram[(self.wram_bank * 0x1000) | (address as usize & 0x0FFF)] = value;
			}
			0xFF00 => self.keypad.wb(value),
			0xFF01..=0xFF02 => self.serial.wb(address, value),
			0xFF04..=0xFF07 => self.timer.wb(address, value),
			0xFF10..=0xFF3F => self.sound.as_mut().map_or((), |s| s.wb(address, value)),
			0xFF46 => self.oamdma(value),
			0xFF4D | 0xFF4F | 0xFF51..=0xFF55 | 0xFF6C | 0xFF70 | 0xFF76..=0xFF77
				if self.gbmode != GbMode::Color => {}
			0xFF72..=0xFF73 | 0xFF75..=0xFF77 if self.gbmode == GbMode::Classic => {}
			0xFF4D => {
				if value & 0x1 == 0x1 {
					self.speed_switch_req = true;
				}
			}
			0xFF51..=0xFF55 => self.hdma_write(address, value),
			0xFF0F => self.intf = value,
			0xFF70 => {
				self.wram_bank = match value & 0x7 {
					0 => 1,
					n => n as usize,
				};
			}
			0xFF72..=0xFF73 => self.undocumented_cgb_regs[address as usize - 0xFF72] = value,
			0xFF75 => self.undocumented_cgb_regs[2] = value,
			0xFF80..=0xFFFE => self.zram[address as usize & 0x007F] = value,
			0xFFFF => self.inte = value,
			_ => {}
		}
	}

	pub fn do_cycle(&mut self, ticks: u32) -> u32 {
		let cpudivider = self.gbspeed as u32;
		let vramticks = self.perform_vramdma();
		let gputicks = ticks / cpudivider + vramticks;
		let cputicks = ticks + vramticks * cpudivider;

		self.timer.do_cycle(cputicks);
		self.intf |= mem::take(&mut self.timer.interrupt);

		self.intf |= mem::take(&mut self.keypad.interrupt);

		self.gpu.do_cycle(gputicks);
		self.intf |= mem::take(&mut self.gpu.interrupt);

		() = self.sound.as_mut().map_or((), |s| s.do_cycle(gputicks));

		self.intf |= mem::take(&mut self.serial.interrupt);

		gputicks
	}

	pub fn ww(&mut self, address: u16, value: u16) {
		self.wb(address, (value & 0xFF) as u8);
		self.wb(address + 1, (value >> 8) as u8);
	}

	pub const fn switch_speed(&mut self) {
		if self.speed_switch_req {
			if matches!(self.gbspeed, GbSpeed::Double) {
				self.gbspeed = GbSpeed::Single;
			} else {
				self.gbspeed = GbSpeed::Double;
			}
		}

		self.speed_switch_req = false;
	}

	fn oamdma(&mut self, value: u8) {
		let base = u16::from(value) << 8;
		for i in 0..0xA0 {
			let b = self.rb(base + i);
			self.wb(0xFE00 + i, b);
		}
	}

	fn hdma_read(&self, a: u16) -> u8 {
		match a {
			0xFF51..=0xFF54 => self.hdma[(a - 0xFF51) as usize],
			0xFF55 => {
				self.hdma_len
					| if matches!(self.hdma_status, DMAType::NoDMA) {
						0x80
					} else {
						0
					}
			}
			_ => panic!("the address {a:04X} should not be handled by hdma_read"),
		}
	}

	fn hdma_write(&mut self, a: u16, v: u8) {
		match a {
			0xFF51 => self.hdma[0] = v,
			0xFF52 => self.hdma[1] = v & 0xF0,
			0xFF53 => self.hdma[2] = v & 0x1F,
			0xFF54 => self.hdma[3] = v & 0xF0,
			0xFF55 => {
				if matches!(self.hdma_status, DMAType::HDMA) {
					if matches!(v & 0x80, 0) {
						self.hdma_status = DMAType::NoDMA;
					}
					return;
				}

				let src = (u16::from(self.hdma[0]) << 8) | u16::from(self.hdma[1]);
				let dst = (u16::from(self.hdma[2]) << 8) | u16::from(self.hdma[3]) | 0x8000;
				assert!(
					(src <= 0x7FF0 || (0xA000..=0xDFF0).contains(&src)),
					"HDMA transfer with illegal start address {src:04X}"
				);

				self.hdma_src = src;
				self.hdma_dst = dst;
				self.hdma_len = v & 0x7F;

				self.hdma_status = if v & 0x80 == 0x80 {
					DMAType::HDMA
				} else {
					DMAType::GDMA
				};
			}
			_ => panic!("the address {a:04X} should not be handled by hdma_write"),
		}
	}

	fn perform_vramdma(&mut self) -> u32 {
		match self.hdma_status {
			DMAType::NoDMA => 0,
			DMAType::GDMA => self.perform_gdma(),
			DMAType::HDMA => self.perform_hdma(),
		}
	}

	fn perform_hdma(&mut self) -> u32 {
		if !self.gpu.may_hdma() {
			return 0;
		}

		self.perform_vramdma_row();

		if matches!(self.hdma_len, 0x7F) {
			self.hdma_status = DMAType::NoDMA;
		}

		8
	}

	fn perform_gdma(&mut self) -> u32 {
		let len = u32::from(self.hdma_len) + 1;
		for _ in 0..len {
			self.perform_vramdma_row();
		}

		self.hdma_status = DMAType::NoDMA;
		len * 8
	}

	fn perform_vramdma_row(&mut self) {
		let mmu_src = self.hdma_src;
		for j in 0..0x10 {
			let b = self.rb(mmu_src + j);
			self.gpu.wb(self.hdma_dst + j, b);
		}
		self.hdma_src += 0x10;
		self.hdma_dst += 0x10;

		if self.hdma_len == 0 {
			self.hdma_len = 0x7F;
		} else {
			self.hdma_len -= 1;
		}
	}
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
enum DMAType {
	NoDMA,
	GDMA,
	HDMA,
}

fn fill_random(slice: &mut [u8], start: u32) {
	const A: u32 = 1_103_515_245;
	const C: u32 = 12345;

	let mut x = start;
	for v in slice.iter_mut() {
		x = x.wrapping_mul(A).wrapping_add(C);
		*v = ((x >> 23) & 0xFF) as u8;
	}
}
