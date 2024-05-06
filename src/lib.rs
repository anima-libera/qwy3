#![allow(clippy::items_after_test_module)]

mod atlas;
mod block_types;
mod camera;
mod chunk_blocks;
mod chunk_loading;
mod chunk_meshing;
mod chunks;
mod cmdline;
mod commands;
mod coords;
mod entities;
mod entity_parts;
mod font;
mod game_init;
mod game_loop;
mod interface;
mod lang;
mod noise;
mod physics;
mod rendering;
mod rendering_init;
mod saves;
mod shaders;
mod simple_meshes;
mod skybox;
mod table_allocator;
mod tasks;
mod texture_gen;
mod threadpool;
mod widgets;
mod world_gen;

pub use game_loop::init_and_run_game_loop;
