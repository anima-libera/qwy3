
# TODO list for Qwy3

- Cascading shadow mapping. Currently there is a single shadow map, but there should be multiple shadow maps corresponding to bigger and bigger areas so that shadows can be rendered even for far away stuff but without using as much resolution as for shadows close to the player.
- Reduce the length of `lib.rs` and the `run` function!
- More info in the README, like control bindings syntax, default controls, command line arguments, etc.
- Collision physics. The player box should not be able to overlap with non-pass-through blocks, but should be able to slide on walls and walk on the floor and stuff. Beware, this is way more difficult to implement in a sane way than it looks.
