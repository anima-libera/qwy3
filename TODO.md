
# TODO list for Qwy3

- Cascading shadow mapping. Currently there is a single shadow map, but there should be multiple shadow maps corresponding to bigger and bigger areas so that shadows can be rendered even for far away stuff but without using as much resolution as for shadows close to the player.
- X-mesh blocks like grass and flowers.
- Multithreading handling of chunks, like generation, meshing and remeshing should be done by other threads than the main thread. A threadpool and threadsafe data structure for the chunk grid are required for this. The threadpool is easy but what about the data structure for the chunks? How to ensure that there wont be too much delay for the main thread to access data in it? Double buffering? I hope it won't be necessary (duplicating large amount of data and copying it often seems like a very bad idea haha). Maybe just a `Mutex` of the blocks and the mesh of each chunk is enough? What about a `RwLock`? The doc says the policy for priority between pending writers and readers depends on the os and could deadlock... Uuh this will require more investigation. `Dashmap` for the hash map also, maybe.
- Reduce the length of `lib.rs` and the `run` function!
- Readme!
- One bind group per uniform seems like a dead end way to deal with uniforms. The default limit for the maximum number of bind groups is 4, and the hardware on my machine supports up to 8. No way we can get anywhere with 8 uniforms, Qwy2 had simple shaders that used 8 uniforms and it would keep increasing with more features and stuff. Maybe putting all the uniforms in a different bindings of a single bing group, or even all in a struct in a signe binding? Maybe there are other better ways to do it?
- Collision physics. The player box should not be able to overlap with non-pass-through blocks, but should be able to slide on walls and walk on the floor and stuff. Beware, this is way more difficult to implement in a sane way than it looks.
