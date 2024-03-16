
# Qwy3

<p align="center"><img src="./pics/name.png" width="25%" alt="Qwy3 name logo kinda"/></>

<p align="center">
Wanna-be Minecraft-like in Rust using <a href="url">Wgpu</a>, very early stage.
</p>

<p align="center"><img src="./pics/pic-02.png" width="95%" alt="Beautiful screenshot"/></>

<p align="center"><img src="./pics/pic-03.png" width="95%" alt="An other nice screenshot"/></>

<p align="center"><img src="./pics/pic-01.png" width="95%" alt="Older screenshot"/></>

### Usage

Examples:
- `cargo run --release -- --threads 8 --gen-dist 210 --gen structures-links-smooth`
- `cargo run --release -- --threads 4 --gen-dist 300 --gen default --seed 3 --chunk-edge 50`

Advice:
- Up the number of `--threads` the game shall use to almost the number of virtual cores of the hardware.
- Experiment with the `--chunk-edge` parameter to see how it impacts the performance while allowing to load larger areas by upping `--gen-dist`.
- Try out the various world generators available (`--gen-names` to display the list).
- Be ready to press P (default key) to disable falling when using world generators that lack a ground, or be ready to fall for some time.
- Read `controls.qwy3_controls` to get a list of controls and see what can be done once the game run.
- Some useful default controls: WASD to move, P to toggle physics (fly) and mouse wheel to go up and down, U to toggle the interface, K to let the mouse escape, left/right click to remove/place blocks.

### Implemented features

- Blocks, chunks, meshes.
- Chunks, loading and unloading so that an area around the player is loaded.
- Infinite-ish world, in all directions including up and down.
- Removing and placing blocks.
- Saving/loading to/from disk, named saves.
- AABB vs voxels collision resolution.
- Multiple world generators, some use a cool and fast structure generation engine.
- Skybox.
- Fog effect (blocks in the distance fading in the skybox).
- Multithreading.
- Command bar (the command language has basically nothing in it yet but the command bar is there).
- Shadows via cascading shadow mapping.
- Configurable controls for most controls.

### Controls

Default controls make sense for QWERTY keyboards. Most controls are configurable by editing the  `controls.qwy3_controls` file (created by the game in the current directory when it doesn't exist yet).

The syntax is intuitive, key names are letters, numbers, or `up`, `down`, `left`, `right`, `space`, `left_shift`, `right_shift`, `tab`, `return`/`enter`; mouse button names are `left`, `right`, `middle`, or numbers.
