# Future of MIDIAnimator

Going forward, I will be rewriting MIDIAnimator to be written in Rust. This is for a couple of reasons
- way, way faster than Python will ever be
- flexibility and modular code (not so tied in with Blender's API)
- use of external application (explanation below)

## Implementation

The new implementation is as follows:
- All MIDI processing will be done using Rust instead of right in Blender's Python enviroment.
- Rust will communicate with React.js for a front-end external application, and will be bundled as an external application with [Tauri](https://github.com/tauri-apps/tauri)
- The external application will have plugin support for implementing plugins for use with other external applications (Blender, Maya, 3DS Max, Cinema 4D, Unreal Engine 5)
- This plugin API can also implement a synchronous playback system between a DAW, the external application and the 3D application
- Having a broader approach allows for more flexibility and a more modular approach.

## New Technologies

- Rust as backend
- React.js for implementing front-end paramters
- [Tauri](https://github.com/tauri-apps/tauri) for packaging up Rust/React application in one package
- Python will still be used to link the external application and 3D application using `socket` over local TCP. Depending on the scripting enviorment for the 3D application, a specific implementation is required to get scene data and convert it into the Rust API.


## Going Forward

### Will development on the main MIDIAnimator add-on stop?
For now, yes. I haven't worked on the main code for quite some time (I have been too busy to commit time to the project), but I've had this dilemma looming over me this entire time. I wanted a easy to use implementation that would work well for beginners and a fast, flexibility implementation for advanced users. I'm still going to want to work out some of the kinks (like how to do in-line scripting for advanced users?) and generally what the entire roadmap will look like. 

For now, the project will be probably broken out into 2 sub-repositories: front/backend using Rust/Tauri, and implementation for plugins. This, could change if I find other viable solutions. 

### Will my projects transfer over to the new MIDIAnimator?
 Probably not. I don't think theres a lot of users using MIDIAnimator, and because of the technical shift in all of the code, it probably will be completely different in how it operates. You will be responsible for poriting code to the new interface, however I am here to assist with technical questions and issues.


 ---

 I look forward to the future of MIDIAnimator (and a new name... which will be coming!)