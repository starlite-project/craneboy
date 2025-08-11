use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Serial {
	data: u8,
	control: u8,
	#[serde(skip)]
	callback: Option<Box<dyn SerialCallback>>,
	pub interrupt: u8,
}

impl Serial {
	pub const fn new() -> Self {
		Self {
			data: 0,
			control: 0,
			callback: None,
			interrupt: 0,
		}
	}

	pub fn with_callback(cb: Box<dyn SerialCallback>) -> Self {
		Self {
			data: 0,
			control: 0,
			callback: Some(cb),
			interrupt: 0,
		}
	}

	pub fn wb(&mut self, a: u16, v: u8) {
		match a {
			0xFF01 => self.data = v,
			0xFF02 => {
				self.control = v;
				if matches!(v & 0x81, 0x81)
					&& let Some(callback) = &mut self.callback
					&& let Some(result) = callback.call(self.data)
				{
					self.data = result;
					self.interrupt = 0x8;
				}
			}
			_ => panic!("serial does not handle write address {a:4X}"),
		}
	}

	pub fn rb(&self, a: u16) -> u8 {
		match a {
			0xFF01 => self.data,
			0xFF02 => self.control | 0b0111_1110,
			_ => panic!("serial does not handle read address {a:4X}"),
		}
	}

	pub fn set_callback(&mut self, cb: Box<dyn SerialCallback>) {
		self.callback = Some(cb);
	}

	pub fn clear_callback(&mut self) {
		self.callback = None;
	}
}

pub trait SerialCallback: Send {
	fn call(&mut self, value: u8) -> Option<u8>;
}
