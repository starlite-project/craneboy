use std::mem;

use blip_buf::BlipBuf;

const WAVE_PATTERN: [[i32; 8]; 4] = [
	[-1, -1, -1, -1, 1, -1, -1, -1],
	[-1, -1, -1, -1, 1, 1, -1, -1],
	[-1, -1, 1, 1, 1, 1, -1, -1],
	[1, 1, 1, 1, -1, -1, 1, 1],
];
const CLOCKS_PER_SECOND: u32 = 1 << 22;
const CLOCKS_PER_FRAME: u32 = CLOCKS_PER_SECOND / 512;
const OUTPUT_SAMPLE_COUNT: usize = 2000;
const SWEEP_DELAY_ZERO_PERIOD: u8 = 8;

const WAVE_INITIAL_DELAY: u32 = 4;

struct VolumeEnvelope {
	period: u8,
	goes_up: bool,
	delay: u8,
	initial_volume: u8,
	volume: u8,
}

impl VolumeEnvelope {
	const fn new() -> Self {
		Self {
			period: 0,
			goes_up: false,
			delay: 0,
			initial_volume: 0,
			volume: 0,
		}
	}

	fn rb(&self, a: u16) -> u8 {
		match a {
			0xFF12 | 0xFF17 | 0xFF21 => {
				((self.initial_volume & 0xF) << 4)
					| if self.goes_up { 0x08 } else { 0 }
					| (self.period & 0x7)
			}
			_ => unreachable!(),
		}
	}

	const fn wb(&mut self, a: u16, v: u8) {
		match a {
			0xFF12 | 0xFF17 | 0xFF21 => {
				self.period = v & 0x7;
				self.goes_up = matches!(v & 0x8, 0x8);
				self.initial_volume = v >> 4;
				self.volume = self.initial_volume;
			}
			0xFF14 | 0xFF19 | 0xFF23 if matches!(v & 0x80, 0x80) => {
				self.delay = self.period;
				self.volume = self.initial_volume;
			}
			_ => {}
		}
	}

	const fn step(&mut self) {
		if self.delay > 1 {
			self.delay -= 1;
		} else if matches!(self.delay, 1) {
			self.delay = self.period;
			if self.goes_up && self.volume < 15 {
				self.volume += 1;
			} else if !self.goes_up && self.volume > 0 {
				self.volume -= 1;
			}
		}
	}
}

struct LengthCounter {
	enabled: bool,
	value: u16,
	max: u16,
}

impl LengthCounter {
	const fn new(max: u16) -> Self {
		Self {
			enabled: false,
			value: 0,
			max,
		}
	}

	const fn is_active(&self) -> bool {
		self.value > 0
	}

	const fn extra_step(frame_step: u8) -> bool {
		matches!(frame_step % 2, 1)
	}

	const fn enable(&mut self, enable: bool, frame_step: u8) {
		let was_enabled = mem::replace(&mut self.enabled, enable);
		if !was_enabled && Self::extra_step(frame_step) {
			self.step();
		}
	}

	const fn set(&mut self, minus_value: u8) {
		self.value = self.max - minus_value as u16;
	}

	const fn trigger(&mut self, frame_step: u8) {
		if matches!(self.value, 0) {
			self.value = self.max;
			if Self::extra_step(frame_step) {
				self.step();
			}
		}
	}

	const fn step(&mut self) {
		if self.enabled && self.value > 0 {
			self.value -= 1;
		}
	}
}

struct SquareChannel {
	active: bool,
	dac_enabled: bool,
	duty: u8,
	phase: u8,
	length: LengthCounter,
	frequency: u16,
	period: u32,
	last_amp: i32,
	delay: u32,
	has_sweep: bool,
	sweep_enabled: bool,
	sweep_frequency: u16,
	sweep_delay: u8,
	sweep_period: u8,
	sweep_shift: u8,
	sweep_negate: bool,
	sweep_did_negate: bool,
	volume_envelope: VolumeEnvelope,
	blip: BlipBuf,
}

impl SquareChannel {
	const fn new(blip: BlipBuf, with_sweep: bool) -> Self {
		Self {
			active: false,
			dac_enabled: false,
			duty: 1,
			phase: 1,
			length: LengthCounter::new(64),
			frequency: 0,
			period: 2048,
			last_amp: 0,
			delay: 0,
			has_sweep: with_sweep,
			sweep_enabled: false,
			sweep_frequency: 0,
			sweep_delay: 0,
			sweep_period: 0,
			sweep_shift: 0,
			sweep_negate: false,
			sweep_did_negate: false,
			volume_envelope: VolumeEnvelope::new(),
			blip,
		}
	}

	const fn on(&self) -> bool {
		self.active
	}

	fn rb(&self, a: u16) -> u8 {
		match a {
			0xFF10 => {
				0x80 | ((self.sweep_period & 0x7) << 4)
					| if self.sweep_negate { 0x8 } else { 0 }
					| (self.sweep_shift & 0x7)
			}
			0xFF11 | 0xFF16 => ((self.duty & 3) << 6) | 0x3F,
			0xFF12 | 0xFF17 => self.volume_envelope.rb(a),
			0xFF13 | 0xFF18 => 0xFF,
			0xFF14 | 0xFF19 => 0x80 | if self.length.enabled { 0x40 } else { 0 } | 0x3F,
			_ => unreachable!(),
		}
	}

	const fn wb(&mut self, a: u16, v: u8, frame_step: u8) {
		match a {
			0xFF10 => {
				self.sweep_period = (v >> 4) & 0x7;
				self.sweep_shift = v & 0x7;
				let old_sweep_negate = mem::replace(&mut self.sweep_negate, matches!(v & 0x8, 0x8));
				if old_sweep_negate && !self.sweep_negate && self.sweep_did_negate {
					self.active = false;
				}

				self.sweep_did_negate = false;
			}
			0xFF11 | 0xFF16 => {
				self.duty = v >> 6;
				self.length.set(v & 0x3F);
			}
			0xFF12 | 0xFF17 => {
				self.dac_enabled = !matches!(v & 0xF8, 0);
				self.active = self.active && self.dac_enabled;
			}
			0xFF13 | 0xFF18 => {
				self.frequency = (self.frequency & 0x0700) | (v as u16);
				self.calculate_period();
			}
			0xFF14 | 0xFF19 => {
				self.frequency = (self.frequency & 0x00FF) | (((v & 0b0000_0111) as u16) << 8);
				self.calculate_period();

				self.length.enable(matches!(v & 0x40, 0x40), frame_step);
				self.active &= self.length.is_active();

				if matches!(v & 0x80, 0x80) {
					if self.dac_enabled {
						self.active = true;
					}

					self.length.trigger(frame_step);

					if self.has_sweep {
						self.sweep_frequency = self.frequency;
						self.sweep_delay = if matches!(self.sweep_period, 0) {
							SWEEP_DELAY_ZERO_PERIOD
						} else {
							self.sweep_period
						};

						self.sweep_enabled = self.sweep_period > 0 || self.sweep_shift > 0;
						if self.sweep_shift > 0 {
							self.sweep_calculate_frequency();
						}
					}
				}
			}
			_ => {}
		}

		self.volume_envelope.wb(a, v);
	}

	fn run(&mut self, start_time: u32, end_time: u32) {
		if !self.active || matches!(self.period, 0) {
			if !matches!(self.last_amp, 0) {
				self.blip.add_delta(start_time, -self.last_amp);
				self.last_amp = 0;
				self.delay = 0;
			}
		} else {
			let mut time = start_time + self.delay;
			let pattern = WAVE_PATTERN[self.duty as usize];
			let volume = i32::from(self.volume_envelope.volume);

			while time < end_time {
				let amp = volume * pattern[self.phase as usize];
				if amp != self.last_amp {
					self.blip.add_delta(time, amp - self.last_amp);
					self.last_amp = amp;
				}

				time += self.period;
				self.phase = (self.phase + 1) % 8;
			}

			self.delay = time - end_time;
		}
	}

	const fn step_sweep(&mut self) {
		debug_assert!(self.has_sweep);

		if self.sweep_delay > 1 {
			self.sweep_delay -= 1;
		} else if matches!(self.sweep_delay, 1) {
			self.sweep_delay = SWEEP_DELAY_ZERO_PERIOD;
		} else {
			self.sweep_delay = self.sweep_period;
			if self.sweep_enabled {
				let new_freq = self.sweep_calculate_frequency();
				if new_freq <= 2047 {
					if !matches!(self.sweep_shift, 0) {
						self.sweep_frequency = new_freq;
						self.frequency = new_freq;
						self.calculate_period();
					}

					self.sweep_calculate_frequency();
				}
			}
		}
	}

	const fn calculate_period(&mut self) {
		if self.frequency > 2047 {
			self.period = 0;
		} else {
			self.period = (2048 - self.frequency as u32) * 4;
		}
	}

	const fn step_length(&mut self) {
		self.length.step();

		self.active &= self.length.is_active();
	}

	const fn sweep_calculate_frequency(&mut self) -> u16 {
		let offset = self.sweep_frequency >> self.sweep_shift;

		let new_freq = if self.sweep_negate {
			self.sweep_did_negate = true;
			self.sweep_frequency.wrapping_sub(offset)
		} else {
			self.sweep_frequency.wrapping_add(offset)
		};

		if new_freq > 2047 {
			self.active = false;
		}

		new_freq
	}
}

struct WaveChannel {
	active: bool,
	dac_enabled: bool,
	length: LengthCounter,
	frequency: u16,
	period: u32,
	last_amp: i32,
	delay: u32,
	volume_shift: u8,
	waveram: [u8; 16],
	current_wave: u8,
	dmg_mode: bool,
	sample_recently_accessed: bool,
	blip: BlipBuf,
}

impl WaveChannel {
	const fn new(blip: BlipBuf, dmg_mode: bool) -> Self {
		Self {
			active: false,
			dac_enabled: false,
			length: LengthCounter::new(256),
			frequency: 0,
			period: 2048,
			last_amp: 0,
			delay: 0,
			volume_shift: 0,
			waveram: [0; 16],
			current_wave: 0,
			dmg_mode,
			sample_recently_accessed: false,
			blip,
		}
	}

	const fn rb(&self, a: u16) -> u8 {
		match a {
			0xFF1A => (if self.dac_enabled { 0x80 } else { 0 }) | 0x7F,
			0xFF1B | 0xFF1D => 0xFF,
			0xFF1C => 0x80 | ((self.volume_shift & 0b11) << 5) | 0x1F,
			0xFF1E => 0x80 | if self.length.enabled { 0x40 } else { 0 } | 0x3F,
			0xFF30..=0xFF3F => {
				if !self.active {
					self.waveram[a as usize - 0xFF30]
				} else if !self.dmg_mode || self.sample_recently_accessed {
					self.waveram[self.current_wave as usize >> 1]
				} else {
					0xFF
				}
			}
			_ => unreachable!(),
		}
	}

	const fn wb(&mut self, a: u16, v: u8, frame_step: u8) {
		match a {
			0xFF1A => {
				self.dac_enabled = matches!(v & 0x80, 0x80);
				self.active = self.active && self.dac_enabled;
			}
			0xFF1B => self.length.set(v),
			0xFF1C => self.volume_shift = (v >> 5) & 0b11,
			0xFF1D => {
				self.frequency = (self.frequency & 0x0700) | (v as u16);
				self.calculate_period();
			}
			_ => {}
		}
	}

	const fn calculate_period(&mut self) {
		if self.frequency > 2048 {
			self.period = 0;
		} else {
			self.period = (2048 - self.frequency as u32) * 2;
		}
	}
}

pub trait AudioPlayer: Send {
	fn play(&mut self, left_channel: &[f32], right_channel: &[f32]);

	fn samples_rate(&self) -> u32;

	fn underflowed(&self) -> bool;
}
