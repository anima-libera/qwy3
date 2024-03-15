//! Managing saves, their directory structures and all.

use crate::ChunkCoords;

/// Represents a save, the directories and files that make a Qwy3 world persistent
/// by keeping its state saved on the disk.
pub(crate) struct Save {
	pub(crate) name: String,
	pub(crate) main_directory: std::path::PathBuf,
	pub(crate) state_file_path: std::path::PathBuf,
	chunks_directory: std::path::PathBuf,
}

impl Save {
	pub(crate) fn create(name: String) -> Save {
		assert!(name.chars().all(|c| c.is_ascii_alphanumeric()));
		let main_directory = {
			let mut main_directory = std::path::PathBuf::new();
			main_directory.push("saves");
			main_directory.push(&name);
			std::fs::create_dir_all(&main_directory).unwrap();
			main_directory
		};
		let state_file_path = {
			let mut chunks_directory = main_directory.clone();
			chunks_directory.push("state");
			chunks_directory
		};
		let chunks_directory = {
			let mut chunks_directory = main_directory.clone();
			chunks_directory.push("chunks");
			std::fs::create_dir_all(&chunks_directory).unwrap();
			chunks_directory
		};
		Save { name, main_directory, state_file_path, chunks_directory }
	}

	pub(crate) fn chunk_file_path(&self, chunk_coords: ChunkCoords) -> std::path::PathBuf {
		let mut path = self.chunks_directory.clone();
		let cgmath::Point3 { x, y, z } = chunk_coords;
		path.push(format!("{x},{y},{z}"));
		path
	}
}
