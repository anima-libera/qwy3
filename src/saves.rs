use crate::ChunkCoords;

pub(crate) struct Save {
	pub(crate) name: String,
	pub(crate) main_directory: std::path::PathBuf,
	chunks_directory: std::path::PathBuf,
}

impl Save {
	pub(crate) fn create(name: String) -> Save {
		assert!(name.chars().all(|c| c.is_ascii_alphanumeric()));
		let mut main_directory = std::path::PathBuf::new();
		main_directory.push("saves");
		main_directory.push(&name);
		std::fs::create_dir_all(&main_directory).unwrap();
		let mut chunks_directory = main_directory.clone();
		chunks_directory.push("chunks");
		std::fs::create_dir_all(&chunks_directory).unwrap();
		Save { name, main_directory, chunks_directory }
	}

	pub(crate) fn chunk_file_path(&self, chunk_coords: ChunkCoords) -> std::path::PathBuf {
		let mut path = self.chunks_directory.clone();
		let cgmath::Point3 { x, y, z } = chunk_coords;
		path.push(format!("{x},{y},{z}"));
		path
	}
}
