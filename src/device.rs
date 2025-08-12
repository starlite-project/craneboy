use serde::{Deserialize, Serialize};

use super::{
	StrResult,
	cpu::CPU,
	gb_mode::GbMode,
	keypad::KeypadKey,
	mbc,
	printer::GbPrinter,
	serial::{self, SerialCallback},
	sound,
};

#[derive(Serialize, Deserialize)]
pub struct Device {
	cpu: CPU,
	save_state: Option<String>,
}

impl Drop for Device {
	fn drop(&mut self) {}
}
