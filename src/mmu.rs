use serde::{Deserialize, Serialize};

use super::{
	StrResult,
	gb_mode::{GbMode, GbSpeed},
	gpu::GPU,
	keypad::Keypad,
	mbc,
	serial::{Serial, SerialCallback},
	timer::Timer,
};
