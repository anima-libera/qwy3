
# Qwy3

Wanna-be Minecraft-like in Rust using [Wgpu](https://wgpu.rs/), very early stage.

<p align="center"><img src="./pics/pic-02.png" width="95%"/></>

<p align="center"><img src="./pics/pic-01.png" width="95%"/></>

### Usage

Examples:
- `cargo run --release -- --threads 8 --gen-dist 210 --gen structures-links-smooth --no-fog`
- `cargo run --release -- --threads 4 --gen-dist 300 --gen default --seed 3 --no-fog --chunk-edge 50`

Advice:
- Up the number of `--threads` the game shall use to almost the number of virtual cores of the hardware.
- Experiment with the `--chunk-edge` parameter to see how it impacts the performance while allowing to load larger areas by upping `--gen-dist`.
- Try out the various world generators available (`--gen-names` to display the list).
- Try removing `--no-fog` to get a fog transparency effect (cool, but reduces the rendred area a bit, and uh it kinda hides how fast the world generation is).
- Read `controls.qwy3_controls` to get a list of controls and see what can be done once the game run.

### Implemented features

- Blocks, chunks, meshes.
- Chunks, loading and unloading so that an area around the player is loaded.
- Removing and placing blocks.
- Multiple world generators, some use a cool and fast structure generation engine.
- Skybox.
- Multithreading.
- Command bar (the command language has basically nothing in it yet but the command bar is there).
- Shadows.
- Configurable controls for most controls.

### Controls

Default controls make sense for QWERTY keyboards. Most controls are configurable by editing the  `controls.qwy3_controls` file (created by the game in the current directory when it doesn't exist yet).

The syntax is intuitive, key names are letters, numbers, or `up`, `down`, `left`, `right`, `space`, `left_shift`, `right_shift`, `tab`, `return`/`enter`; mouse button names are `left`, `right`, `middle`, or numbers.
