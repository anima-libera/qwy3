use clap::Parser;

use crate::world_gen::WhichWorldGenerator;

#[derive(Parser)]
#[command(color = clap::ColorChoice::Auto)]
pub struct CommandLineSettings {
	/// Number of worker threads in the threadpool.
	#[arg(long = "threads", short = 't', default_value_t = 12, value_name = "N")]
	pub number_of_threads: u32,

	/// Does the game should close itself after one frame?
	#[arg(long = "close")]
	pub close_after_one_frame: bool,

	/// Verbose mode. Kinda does nothing for now, sorry >_<.
	#[arg(long)]
	pub verbose: bool,

	/// Outputs the texture atlas as a PNG file.
	#[arg(long)]
	pub output_atlas: bool,

	/// World generation seed.
	#[arg(long = "seed", short = 's', default_value_t = 0, value_name = "SEED")]
	pub world_gen_seed: i32,

	/// Selection of one world generator.
	#[arg(
		long = "gen",
		short = 'g',
		value_enum,
		default_value_t = WhichWorldGenerator::Default,
		value_name = "GENERATOR_NAME",
		hide_possible_values = true,
	)]
	pub which_world_generator: WhichWorldGenerator,

	/// Prints the list of available world generators.
	#[arg(long = "gen-names")]
	pub display_world_generator_possible_names: bool,

	/// Loading distance in blocks.
	#[arg(
		long = "gen-dist",
		short = 'd',
		default_value_t = 190.0,
		value_name = "LENGTH"
	)]
	pub loading_distance: f32,

	/// Length of the edge of the chunks, in blocks.
	#[arg(long, default_value_t = 20, value_name = "LENGTH")]
	pub chunk_edge: u32,

	/// Enables the fog from the start.
	#[arg(long)]
	pub fog: bool,

	/// Runs a specific Qwy Script test instead of running the game.
	#[arg(long)]
	pub test_lang: Option<u32>,
}

pub fn parse_command_line_arguments() -> CommandLineSettings {
	CommandLineSettings::parse()
}

pub fn display_world_generator_names() {
	use clap::ValueEnum;
	for variant in WhichWorldGenerator::value_variants() {
		if let Some(possible_value) = variant.to_possible_value() {
			println!("{}", possible_value.get_name());
		}
	}
}
