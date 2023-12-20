use clap::Parser;

use crate::world_gen::WhichWorldGenerator;

pub struct CommandLineSettings {
	pub number_of_threads: u32,
	pub close_after_one_frame: bool,
	pub verbose: bool,
	pub output_atlas: bool,
	pub world_gen_seed: i32,
	pub which_world_generator: WhichWorldGenerator,
	pub loading_distance: f32,
	pub chunk_edge: u32,

	pub test_lang: Option<u32>,
}

#[derive(Parser)]
struct ClapParsed {
	/// Number of worker threads in the threadpool.
	#[arg(long)]
	threads: Option<u32>,

	/// Does the game should close itself after one frame?
	#[arg(long)]
	close: bool,

	/// Verbose mode. Kinda does nothing for now, sorry >_<.
	#[arg(long)]
	verbose: bool,

	/// Outputs the texture atlas as a PNG file.
	#[arg(long)]
	output_atlas: bool,

	/// World generation seed.
	#[arg(long)]
	seed: Option<i32>,

	/*
	gen: WhichWorldGenerator,
	*/
	/// Loading distance in blocks.
	#[arg(long)]
	gen_dist: Option<f32>,

	/// Length of the edge of the chunks, in blocks.
	#[arg(long)]
	chunk_edge: Option<u32>,

	/// Runs a specific Qwy Script test instead of running the game.
	#[arg(long)]
	test_lang: Option<u32>,
}

pub fn parse_command_line_arguments() -> CommandLineSettings {
	let parsed = ClapParsed::parse();

	CommandLineSettings {
		number_of_threads: parsed.threads.unwrap_or(12),
		close_after_one_frame: parsed.close,
		verbose: parsed.verbose,
		output_atlas: parsed.output_atlas,
		world_gen_seed: parsed.seed.unwrap_or(0),
		which_world_generator: WhichWorldGenerator::Default,
		loading_distance: parsed.gen_dist.unwrap_or(190.0),
		chunk_edge: parsed.chunk_edge.unwrap_or(20),
		test_lang: parsed.test_lang,
	}
}
