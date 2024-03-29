//! Managing saves, their directory structures and all.

use std::{
	collections::HashMap,
	io::{Read, Write},
	path::PathBuf,
	sync::{Arc, RwLock},
};

use rustc_hash::FxHashMap;

use crate::coords::{ChunkCoords, OrientedAxis};

/// Represents a save, the directories and files that make a Qwy3 world persistent
/// by keeping its state saved on the disk.
pub(crate) struct Save {
	pub(crate) name: String,
	pub(crate) main_directory: std::path::PathBuf,
	pub(crate) state_file_path: std::path::PathBuf,
	chunks_directory: std::path::PathBuf,
	pub(crate) textures_directory: std::path::PathBuf,
	pub(crate) atlas_texture_file_path: std::path::PathBuf,

	/// Super mega thread safe file i/o manager that enforces rust's borrow cheking rules on files.
	file_io_table: RwLock<FxHashMap<PathBuf, Arc<RwLock<FileIoToken>>>>,
}

pub(crate) enum WhichChunkFile {
	Blocks,
	Entities,
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
		let textures_directory = {
			let mut chunks_directory = main_directory.clone();
			chunks_directory.push("textures");
			std::fs::create_dir_all(&chunks_directory).unwrap();
			chunks_directory
		};
		let atlas_texture_file_path = {
			let mut chunks_directory = textures_directory.clone();
			chunks_directory.push("atlas.png");
			chunks_directory
		};

		let file_io_table = RwLock::new(HashMap::default());

		Save {
			name,
			main_directory,
			state_file_path,
			chunks_directory,
			textures_directory,
			atlas_texture_file_path,
			file_io_table,
		}
	}

	pub(crate) fn chunk_file_path(
		&self,
		chunk_coords: ChunkCoords,
		which_file: WhichChunkFile,
	) -> std::path::PathBuf {
		let mut path = self.chunks_directory.clone();
		let cgmath::Point3 { x, y, z } = chunk_coords;
		let which_file_char = match which_file {
			WhichChunkFile::Blocks => 'b',
			WhichChunkFile::Entities => 'e',
		};
		path.push(format!("{x},{y},{z},{which_file_char}",));
		path
	}

	pub(crate) fn skybox_face_texture_file_path(
		&self,
		face_direction: OrientedAxis,
	) -> std::path::PathBuf {
		let mut path = self.textures_directory.clone();
		let sign = face_direction.orientation.as_char();
		let axis = face_direction.axis.as_char();
		path.push(format!("{sign}{axis}.png"));
		path
	}

	pub(crate) fn get_file_io(&self, path: PathBuf) -> SyncFileIo {
		// Thread-safely make sure the path has an entry in the table.
		// If we can do it with just reading, then very good, else we write it in if necessary.
		if let Ok(table) = self.file_io_table.try_read() {
			if let Some(token) = table.get(&path) {
				return SyncFileIo { path, token: Arc::clone(token) };
			}
		}
		let mut table_writer = self.file_io_table.write().unwrap();
		let token = Arc::clone(
			table_writer.entry(path.clone()).or_insert_with(|| Arc::new(RwLock::new(FileIoToken {}))),
		);
		SyncFileIo { path, token }
	}
}

struct FileIoToken {}

pub(crate) struct SyncFileIo {
	path: PathBuf,
	token: Arc<RwLock<FileIoToken>>,
}

impl SyncFileIo {
	pub(crate) fn write(&self, data: &[u8]) {
		let _guard = self.token.write().unwrap();
		let mut file = std::fs::File::create(&self.path).unwrap();
		file.write_all(data).unwrap();
	}

	pub(crate) fn read(&self, delete_file_after_read: bool) -> Option<Vec<u8>> {
		let mut data = vec![];
		{
			let _guard = self.token.read().unwrap();
			let mut file = std::fs::File::open(&self.path).ok()?;
			file.read_to_end(&mut data).unwrap();
		}
		if delete_file_after_read {
			let _guard = self.token.write().unwrap();
			std::fs::remove_file(&self.path).ok();
			// Note: `remove_file` doc says that the file may not be immediately removed, which would
			// be a problem if it could happen after the write guard is dropped. However, the doc says
			// that this can happen because of "other open file descriptors", which should not exist
			// due to our write guard, so we should be safe...
		}
		Some(data)
	}
}
