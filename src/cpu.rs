use serde::{Deserialize, Serialize};

use super::{
	SerialCallback, StrResult, mbc,
	mmu::MMU,
	registers::{CpuFlags, Registers},
};

#[derive(Serialize, Deserialize)]
pub struct CPU {
	reg: Registers,
	pub mmu: MMU,
	halted: bool,
	halt_bug: bool,
	ime: bool,
	setdi: u32,
	setei: u32,
}

impl CPU {
	pub fn classic(
		cart: Box<dyn mbc::MBC + 'static>,
		serial_callback: Option<Box<dyn SerialCallback>>,
	) -> StrResult<Self> {
		let mmu = MMU::classic(cart, serial_callback)?;
		let registers = Registers::new(mmu.gbmode);

		Ok(Self {
			reg: registers,
			halt_bug: false,
			halted: false,
			ime: true,
			setdi: 0,
			setei: 0,
			mmu,
		})
	}

	pub fn cgb(
		cart: Box<dyn mbc::MBC + 'static>,
		serial_callback: Option<Box<dyn SerialCallback>>,
	) -> StrResult<Self> {
		let mmu = MMU::cgb(cart, serial_callback)?;
		let registers = Registers::new(mmu.gbmode);
		Ok(Self {
			reg: registers,
			halt_bug: false,
			halted: false,
			ime: true,
			setdi: 0,
			setei: 0,
			mmu,
		})
	}

	pub fn do_cycle(&mut self) -> u32 {}

	fn get_ticks(&mut self) -> u32 {
		self.update_ime();

		match self.handle_interrupt() {
			0 => {}
			n => return n,
		};

		if self.halted { 1 } else { self.call() }
	}

    fn handle_interrupt(&mut self) -> u32 {
        if !self.ime && !self.halted {
            return 0;
        }

        let triggered = self.mmu.inte & self.mmu.intf & 0x1F;
        if matches!(triggered, 0) {
            return 0;
        }

        self.halted = false;
        if !self.ime {
            return 0;
        }

        self.ime = false;

        let n =triggered.trailing_zeros();
        if n >= 5 {
            panic!("invalid interrupt triggered");
        }

        self.mmu.intf &= !(1 << n);
        let pc = self.reg.pc;
        self.push_stack(pc);
        self.reg.pc = 0x0040 | ((n as u16) << 3);

        4
    }

    fn push_stack(&mut self, value: u16) {
        self.reg.sp = self.reg.sp.wrapping_sub(2);
        self.mmu.ww(self.reg.sp, value);
    }

	fn update_ime(&mut self) {
		self.setdi = match self.setdi {
			2 => 1,
			1 => {
				self.ime = false;
				0
			}
			_ => 0,
		};

		self.setei = match self.setei {
			2 => 1,
			1 => {
				self.ime = true;
				0
			}
			_ => 0,
		}
	}

	fn call(&mut self) -> u32 {
		todo!()
	}
}
