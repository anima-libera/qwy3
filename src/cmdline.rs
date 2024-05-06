use clap::Parser;

use crate::{game_init::PlayingMode, world_gen::WhichWorldGenerator};

#[derive(Parser)]
#[command(color = clap::ColorChoice::Auto)]
pub(crate) struct CommandLineSettings {
	/// Number of worker threads in the threadpool.
	#[arg(long = "threads", short = 't', default_value_t = 12, value_name = "N")]
	pub(crate) number_of_threads: u32,

	/// Does the game should close itself after one frame?
	#[arg(long = "close")]
	pub(crate) close_after_one_frame: bool,

	/// Verbose mode. Kinda does nothing for now, sorry >_<.
	#[arg(long)]
	pub(crate) verbose: bool,

	/// Outputs the texture atlas as a PNG file.
	#[arg(long)]
	pub(crate) output_atlas: bool,

	/// World generation seed.
	#[arg(long = "seed", value_name = "SEED")]
	pub(crate) world_gen_seed: Option<i32>,

	/// Selection of one world generator.
	#[arg(
		long = "gen",
		short = 'g',
		value_enum,
		default_value_t = WhichWorldGenerator::Default,
		value_name = "GENERATOR_NAME",
		hide_possible_values = true,
	)]
	pub(crate) which_world_generator: WhichWorldGenerator,

	/// Prints the list of available world generators.
	#[arg(long = "gen-names")]
	pub(crate) display_world_generator_possible_names: bool,

	/// Loading distance in blocks.
	#[arg(
		long = "gen-dist",
		short = 'd',
		default_value_t = 190.0,
		value_name = "LENGTH"
	)]
	pub(crate) loading_distance: f32,

	/// Length of the edge of the chunks, in blocks.
	#[arg(long, default_value_t = 20, value_name = "LENGTH")]
	pub(crate) chunk_edge: u32,

	/// Enables fullscreen from the start.
	#[arg(long)]
	pub(crate) fullscreen: bool,

	/// Disables V-Sync from the start.
	#[arg(long)]
	pub(crate) no_vsync: bool,

	/// Limit FPS to an arbitrary rate.
	#[arg(long, value_name = "MAX_FRAMERATE")]
	pub(crate) max_fps: Option<i32>,

	/// Disables the fog from the start.
	#[arg(long)]
	pub(crate) no_fog: bool,

	/// Thickness of the foggy area.
	#[arg(long, default_value_t = 60.0, value_name = "LENGTH")]
	pub(crate) fog_margin: f32,

	/// Name by which the save is identified and retrieved/created.
	#[arg(long = "save", short = 's', value_name = "NAME")]
	pub(crate) save_name: Option<String>,

	/// Only save modified chunks (smaller save size, but no faster load time).
	#[arg(long = "only-modified")]
	pub(crate) only_save_modified_chunks: bool,

	/// Selection of the playing mode, `free` or `play`.
	#[arg(
		long = "mode",
		short = 'm',
		value_enum,
		default_value_t = PlayingMode::Free,
		value_name = "PLAYING_MODE",
		hide_possible_values = true,
	)]
	pub(crate) playing_mode: PlayingMode,

	/// Runs a specific Qwy Script test instead of running the game.
	#[arg(long)]
	pub(crate) test_lang: Option<u32>,
}

pub(crate) fn parse_command_line_arguments() -> CommandLineSettings {
	CommandLineSettings::parse()
}

pub(crate) fn display_world_generator_names() {
	use clap::ValueEnum;
	for variant in WhichWorldGenerator::value_variants() {
		if let Some(possible_value) = variant.to_possible_value() {
			println!("{}", possible_value.get_name());
		}
	}
}
