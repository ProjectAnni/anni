# anni-playback

A simple audio playback library based on [SimpleAudio](https://github.com/erikas-taroza/simple_audio).

## What's the difference?

We've changed the following parts:

1. Removed Media Control  
   As it is a simple playback library, controls should be implemented outside it.
2. Removed flutter_rust_bridge related code
3. Removed `update_*` callbacks and setters
   It can be implemented by simply listening events emitted by `Control::event_handler()`.
4. Removed the `Player`  
   Users of this library can follow the example and write their own `Player` struct.