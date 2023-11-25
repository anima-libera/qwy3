
# TODO list for Qwy3

## Graphics

- Cascading shadow mapping. Currently there is a single shadow map, but there should be multiple shadow maps corresponding to bigger and bigger areas so that shadows can be rendered even for far away stuff but without using as much resolution as for shadows close to the player.
- Skybox (procedurally generated, maybe in the style of ouput 20 of noizebra).
  - Make the terrain fade out into the skybox when far enough. Instead of a fog effect that makes the terrain get more and more painted in some plain color as it gets far, it should instead get more and more transparent to let the skybox show.
- Sun. It could look like a 4-branch star in the style of star effects in Kill la Kill.
- God rays effect when looking at sun. I recall it can be done by rendering just the sun in white and all the world in black and doing some motion blur on the result, maybe?
- Glow effect.

## UI

- Maybe introduce type aliases to better label weather we are after or before the correction by `2.0/window_with`.

## Command Language

- Add commands to interface with the game.
- Add strings.
- Add a logging function.
- Add a way to define new typed global variables.
- Add a way to set variables.
- Add floating point numbers.

## World

- Procedurally generated types of block.
  - Procedurally generated textures for these types of blocks.
  - Procedurally generated properties.
- Generation of (procedurally generated types of) structures.
  - Structure types are procedurally generated (meaning that a type of tree found in a world will not be found in other worlds).
    - This can be done by making the structure type be an algorithm that can be procedurally generated (it is just a tree).
  - We have to make sure that the world generation remains the same no matter the order in which chunks are generated, and is not influenced by the grid of chunk (i.e. if the chunks were of a different size and with a different offset then the would generation would generate the same world). This is a problem when for example the world generation (that must act in a way that ignores chunks in its design of the world) decides to generate a structure with some blocks of the structure in a chunk and some blocks in an other chunk... Qwy2 solved this by making the structure generation a separate step from the terrain generation, and making sure that structure generation in a chunk only happens when we have the terrain generated in the whole 3x3x3 chunk cube (but i mean terrain generation is slow enough, we don't really want a whole thick 1-chunk layer of terrain-generated chunks that we can't generate the structures in because they are on the surface of the generated area >.<).
  - **IDEA:** We have to make sure that the terrain generation is querriable and deterministic (not very hard, we just have to keep using noises like we have been in `world_gen.rs`). Then, we consider a 3D grid of cubes, like in the world generator that generates balls (one per cell of the grid). Each cell gives (via noises) a number N of structs that it will attempt to generate, and then we querry a noise in this cell N times to get N block coords in the cell (and an index that indicates a structure type), so we have N structures to attempt to generate in this cell (we have their coords and types). A structure has a bounded box of some size in which it can place blocks, so when we generate a chunk we may have to generate the structs that start from outside of the chunk (even from outside of the cells that overlap with the chunk if some such cells are near enough). A struct type is a (one day procedurally generated) algorithm that can querry the terrain generation (which is deterministic and so can be querried even outside of the generated chunks without too much calculations and memory usage overhead) and can generate a list of modifications to the terrain (like "place a block of some type at some coords" or "break (=replace by air) the block at some coords", etc.). The modifications that are about blocks in the chunk we are generating are the ones that we are intrested in, we apply them to the chunk and discard the others. To handle the conflicting modifications (modifications from different structs on the same block), we just have to decide on a way to totally order all the structs of the world (like we can decide on the lexicographic order of (z, x, y) where (x, y, z) are the coords of their origin point).

## Gameplay

- Collision physics. The player box should not be able to overlap with non-pass-through blocks, but should be able to slide on walls and walk on the floor and stuff. Beware, this is way more difficult to implement in a sane way than it looks.
- Entities!!
  - Particles, like when breaking blocks.
  - Falling blocks.
  - Animals >w< or something.
- Liquids!!
  - Pools of liquid, keeping track of the volume of the pool, all the block coordinates that has some of it, the height of the liquid (z-coord of its surface), etc.
  - Handle the case when a pool of liquid has to flow due to a hole in a neighboring block.
  - Handle gigantic pools of liquid (like a sea or an ocean) that cannot be generated all at once and all and that we can safely consider to contain an infinite amount of liquid.
- Magic system!!
  - Runes, typing runes, casting.
  - Procedural grammar and mapping of elementary spells to their effects.
    - Make sure there are plenty of effects thta can be generated procedurally in the elementary spell map.
  - Mana.
  - Etc. (magic circles, engraving runes on blocks, etc.)

## Other

- Reduce the length of `lib.rs` and the `run` function!
- More info in the README, like control bindings syntax, default controls, command line arguments, etc.

