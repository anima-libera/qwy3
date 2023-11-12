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

pub fn parse_command_line_arguments() -> CommandLineSettings {
	let mut number_of_threads = 12;
	let mut close_after_one_frame = false;
	let mut verbose = false;
	let mut output_atlas = false;
	let mut world_gen_seed = 0;
	let mut which_world_generator = WhichWorldGenerator::Default;
	let mut loading_distance = 190.0;
	let mut chunk_edge = 20;
	let mut test_lang = None;

	let mut args = std::env::args().enumerate();
	args.next(); // Path to binary.
	while let Some((arg_index, arg_name)) = args.next() {
		match arg_name.as_str() {
			"--threads" => match args.next().map(|(second_index, second_arg)| {
				let parsing_result = str::parse::<u32>(&second_arg);
				(second_index, second_arg, parsing_result)
			}) {
				Some((_second_index, _second_arg, Ok(number))) => number_of_threads = number,
				Some((second_index, second_arg, Err(parsing_error))) => {
					println!(
						"Error in command line arguments at argument {second_index}: \
						Argument \"--threads\" is expected to be followed by an unsigned 32-bits \
						integer argument, but parsing of \"{second_arg}\" failed: {parsing_error}"
					);
				},
				None => {
					println!(
						"Error in command line arguments at the end: \
						Argument \"--threads\" is expected to be followed by an unsigned 32-bits \
						integer argument, but no argument followed"
					);
				},
			},
			"--close-after-one-frame" => {
				println!("Will close after one frame");
				close_after_one_frame = true;
			},
			"--verbose" => {
				verbose = true;
			},
			"--output-atlas" => {
				output_atlas = true;
			},
			"--seed" => match args.next().map(|(second_index, second_arg)| {
				let parsing_result = str::parse::<i32>(&second_arg);
				(second_index, second_arg, parsing_result)
			}) {
				Some((_second_index, _second_arg, Ok(number))) => world_gen_seed = number,
				Some((second_index, second_arg, Err(parsing_error))) => {
					println!(
						"Error in command line arguments at argument {second_index}: \
						Argument \"--seed\" is expected to be followed by an signed 32-bits \
						integer argument, but parsing of \"{second_arg}\" failed: {parsing_error}"
					);
				},
				None => {
					println!(
						"Error in command line arguments at the end: \
						Argument \"--seed\" is expected to be followed by an unsigned 32-bits \
						integer argument, but no argument followed"
					);
				},
			},
			"--gen" => {
				match args.next().as_ref().map(|(second_index, second_arg)| {
					let which_world_gen = WhichWorldGenerator::from_name(second_arg);
					(second_index, second_arg, which_world_gen)
				}) {
					Some((_second_index, _second_arg, Some(which_world_gen))) => {
						which_world_generator = which_world_gen;
					},
					Some((second_index, unknown_name, None)) => {
						println!(
							"Error in command line arguments at argument {second_index}: \
							Argument \"--gen\" is expected to be followed by a world generator name, \
							but \"{unknown_name}\" is not a knonw generator name"
						);
						println!("Here is the list of possible generator names:");
						for variant in enum_iterator::all::<WhichWorldGenerator>() {
							let name = variant.name();
							println!(" - {name}");
						}
					},
					None => {
						println!(
							"Error in command line arguments at the end: \
							Argument \"--gen\" is expected to be followed by a world generator name, \
							but no argument followed"
						);
					},
				}
			},
			"--gen-dist" => match args.next().map(|(second_index, second_arg)| {
				let parsing_result = str::parse::<u32>(&second_arg);
				(second_index, second_arg, parsing_result)
			}) {
				Some((_second_index, _second_arg, Ok(number))) => loading_distance = number as f32,
				Some((second_index, second_arg, Err(parsing_error))) => {
					println!(
						"Error in command line arguments at argument {second_index}: \
						Argument \"--gen-dist\" is expected to be followed by an unsigned 32-bits \
						integer argument, but parsing of \"{second_arg}\" failed: {parsing_error}"
					);
				},
				None => {
					println!(
						"Error in command line arguments at the end: \
						Argument \"--gen-dist\" is expected to be followed by an unsigned 32-bits \
						integer argument, but no argument followed"
					);
				},
			},
			"--chunk-edge" => match args.next().map(|(second_index, second_arg)| {
				let parsing_result = str::parse::<u32>(&second_arg);
				(second_index, second_arg, parsing_result)
			}) {
				Some((_second_index, _second_arg, Ok(number))) => chunk_edge = number,
				Some((second_index, second_arg, Err(parsing_error))) => {
					println!(
						"Error in command line arguments at argument {second_index}: \
						Argument \"--chunk-edge\" is expected to be followed by an unsigned 32-bits \
						integer argument, but parsing of \"{second_arg}\" failed: {parsing_error}"
					);
				},
				None => {
					println!(
						"Error in command line arguments at the end: \
						Argument \"--chunk-edge\" is expected to be followed by an unsigned 32-bits \
						integer argument, but no argument followed"
					);
				},
			},
			"--test-lang" => match args.next().map(|(second_index, second_arg)| {
				let parsing_result = str::parse::<u32>(&second_arg);
				(second_index, second_arg, parsing_result)
			}) {
				Some((_second_index, _second_arg, Ok(number))) => test_lang = Some(number),
				Some((second_index, second_arg, Err(parsing_error))) => {
					println!(
						"Error in command line arguments at argument {second_index}: \
						Argument \"--test-lang\" is expected to be followed by an unsigned 32-bits \
						integer argument, but parsing of \"{second_arg}\" failed: {parsing_error}"
					);
				},
				None => {
					println!(
						"Error in command line arguments at the end: \
						Argument \"--test-lang\" is expected to be followed by an unsigned 32-bits \
						integer argument, but no argument followed"
					);
				},
			},
			unknown_arg_name => {
				println!(
					"Error in command line arguments at argument {arg_index}: \
					Argument name \"{unknown_arg_name}\" is unknown"
				);
			},
		}
	}

	CommandLineSettings {
		number_of_threads,
		close_after_one_frame,
		verbose,
		output_atlas,
		world_gen_seed,
		which_world_generator,
		loading_distance,
		chunk_edge,
		test_lang,
	}
}
