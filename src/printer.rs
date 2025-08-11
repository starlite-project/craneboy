use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::SerialCallback;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct GbPrinter {
	status: u8,
	state: u32,
	#[serde(with = "serde_arrays")]
	data: [u8; 0x280 * 9],
	#[serde(with = "serde_arrays")]
	packet: [u8; 0x400],
	count: usize,
	datacount: usize,
	datasize: usize,
	result: u8,
	printcount: u8,
}

impl GbPrinter {
	pub const fn new() -> Self {
		Self {
			status: 0,
			state: 0,
			data: [0; 0x280 * 9],
			packet: [0; 0x400],
			count: 0,
			datacount: 0,
			datasize: 0,
			result: 0,
			printcount: 0,
		}
	}

	fn check_crc(&self) -> bool {
		let mut crc = 0u16;
		for i in 2..(6 + self.datasize) {
			crc = crc.wrapping_add(u16::from(self.packet[i]));
		}

		let msgcrc = u16::from(self.packet[6 + self.datasize])
			.wrapping_add(u16::from(self.packet[7 + self.datasize]) << 8);

		crc == msgcrc
	}

	const fn reset(&mut self) {
		self.state = 0;
		self.datasize = 0;
		self.datacount = 0;
		self.count = 0;
		self.status = 0;
		self.result = 0;
	}

	fn show(&mut self) {
		match self.show_inner() {
			Ok(filename) => println!("print saved successfully to {}", filename.display()),
			Err(e) => println!("error saving print: {e}"),
		}
	}

	fn show_inner(&mut self) -> ::std::io::Result<PathBuf> {
		use std::{fs::OpenOptions, io::Write};

		const OUT_DIR: &str = env!("OUT_DIR");

		let filename = format!("craneboy_print_{:03}.pgm", self.printcount);
		self.printcount += 1;

		let path = PathBuf::from(OUT_DIR).join(filename);

		let image_height = self.datacount / 40;
		if matches!(image_height, 0) {
			return Ok(path);
		}

		let mut f = OpenOptions::new()
			.create(true)
			.write(true)
			.truncate(true)
			.open(&path)?;

		#[expect(clippy::write_with_newline)]
		write!(f, "P5 160 {image_height} 3\n")?;

		let palbyte = self.packet[8];
		let palette = [
			3 - (palbyte & 3),
			3 - ((palbyte >> 2) & 3),
			3 - ((palbyte >> 4) & 3),
			3 - ((palbyte >> 6) & 3),
		];

		for y in 0..image_height {
			for x in 0..160 {
				let tilenumber = ((y >> 3) * 20) + (x >> 3);
				let tileoffset = tilenumber * 16 + (y & 7) * 2;
				let bx = 7 - (x & 7);

				let colorindex = ((self.data[tileoffset] >> bx) & 1)
					| (((self.data[tileoffset + 1] >> bx) << 1) & 2);

				f.write_all(&[palette[colorindex as usize]])?;
			}
		}

		Ok(path)
	}

	fn receive(&mut self) {
		if matches!(self.packet[3], 0) {
			for i in 0..self.datasize {
				self.data[self.datacount + i] = self.packet[6 + i];
			}

			self.datacount += self.datasize;
		} else {
			let mut dataidx = 6;
			let mut destidx = self.datacount;

			while dataidx - 6 < self.datasize {
				let control = self.packet[dataidx];
				dataidx += 1;

				if matches!(control & 0x80, 0) {
					let curlen = (control + 1) as usize;
					for i in 0..curlen {
						self.data[destidx + i] = self.packet[dataidx + i];
					}

					destidx += curlen;
					dataidx += curlen;
				} else {
					let curlen = ((control & 0x7F) + 2) as usize;
					for i in 0..curlen {
						self.data[destidx + i] = self.packet[dataidx];
					}

					dataidx += 1;
					destidx += curlen;
				}
			}

			self.datacount = destidx;
		}
	}

	fn command(&mut self) {
		match self.packet[2] {
			0x01 => {
				self.datacount = 0;
				self.status = 0;
			}
			0x02 => self.show(),
			0x04 => self.receive(),
			_ => {}
		}
	}

	pub fn send(&mut self, v: u8) -> u8 {
		self.packet[self.count] = v;
		self.count += 1;

		match self.state {
			0 => {
				if matches!(v, 0x88) {
					self.state = 1;
				} else {
					self.reset();
				}
			}
			1 => {
				if matches!(v, 0x33) {
					self.state = 2;
				} else {
					self.reset();
				}
			}
			2 => {
				if matches!(self.count, 6) {
					self.datasize = self.packet[4] as usize + ((self.packet[5] as usize) << 8);
					if self.datasize > 0 {
						self.state = 3;
					} else {
						self.state = 4;
					}
				}
			}
			3 => {
				if self.count == self.datasize + 6 {
					self.state = 4;
				}
			}
			4 => self.state = 5,
			5 => {
				if self.check_crc() {
					self.command();
				}

				self.state = 6;
			}
			6 => {
				self.result = 0x81;
				self.state = 7;
			}
			7 => {
				self.result = self.status;
				self.state = 0;
				self.count = 0;
			}
			_ => self.reset(),
		}

		self.result
	}
}

impl SerialCallback for GbPrinter {
	fn call(&mut self, value: u8) -> Option<u8> {
		Some(self.send(value))
	}
}
