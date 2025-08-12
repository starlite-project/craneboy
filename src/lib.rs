#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg))]
#![allow(clippy::upper_case_acronyms)]

mod cpu;
mod device;
mod gb_mode;
mod gpu;
mod keypad;
mod mbc;
mod mmu;
mod printer;
mod registers;
mod serial;
mod sound;
mod timer;

pub use self::{
	gpu::{SCREEN_H, SCREEN_W},
	keypad::KeypadKey,
	serial::SerialCallback,
	sound::AudioPlayer,
};

pub type StrResult<T> = Result<T, &'static str>;
