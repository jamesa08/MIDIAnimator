(getting_started)=

# Getting Started

To start writing your own MIDI Animation programs, first, you will need a scene to work with. You must create objects and have motion in mind
that you will want to animate. For example, if you have a set of cubes, you might want to animate their Z location. 

```{note}
It is reccomended that you keep your scene organized and tidy. Make good names for collections and objects.
```
<hr>

## Step 1: Assign Note Numbers to Objects

Currently, only 1 note number per object is supported, however more than one object can have the same note number in a collection.

To assign note numbers to objects, open up the N-key panel in the 3D workspace. Click on the `MIDIAnimator` tab. When you click on an object, you will see the "Edit Object Information" panel pop up, in which you can assign a note number to the object.

Take for example this cube. It is assigned note number 58.
![cube_note_example](https://raw.githubusercontent.com/jamesa08/MIDIAnimatorDocs/main/docs/images/cube_note_example.png)

### Batch Assigning Note Numbers

Instead of assigning note numbers 1 by 1, you can also assign them in a batch script.

In the N-key panel, under `Assign Notes to Objects`, you input note numbers into a list and pick the collection where the objects are.

To know which note number gets each object, you can use the naming syntax `{objectName}_{noteNumber}`:
   - `objectName` is subjective (it is ignored). 
   - `noteNumber` is the note number for the object.


*If the `Note Number List` field is left blank, it will automatically assign the note number that is in the name field.*


Here is an example of a properly-named collection of objects ready to be assigned note numbers.
![cube_sorting_syntax](https://raw.githubusercontent.com/jamesa08/MIDIAnimatorDocs/main/docs/images/cube_sorting_syntax.png)

Alternatively, you can have the objects sorted by their current names and have the objects be sequentially assigned by checking `Sort By Name`.

When you hit `Run`, you will see a preview on how the note numbers will be assigned.
![cube_sorting_preview](https://raw.githubusercontent.com/jamesa08/MIDIAnimatorDocs/main/docs/images/cube_sorting_preview.png)

<hr>

This entire panel can be controlled via code for batch assignment.<br>
Sample script:
```python
import bpy

scene = bpy.context.scene
scene.midi.quick_note_number_list = "[60, 61, 62, 63, 64]"
scene.midi.quick_obj_col = bpy.data.collections['Cubes']
bpy.ops.scene.quick_add_props()
```

<hr>

## Step 2: Assigning Object-Level Animations to Objects
To assign animations to objects, right click the object you want to assign an animation to, and look in the N-key panel in the 3D viewport.

![cube_anim_properties](https://raw.githubusercontent.com/jamesa08/MIDIAnimatorDocs/main/docs/images/cube_anim_properties.png)

Currently, there is only 1 type of object-level animation, which is called "Keyframed". Keyframed takes an object as input, analyzes it's FCurves, and duplicates the animation across the timeline. 

*In future updates, there will be more object-level types, and they will be explained.*

```{note}
Keep in mind different instrument-level animation types (such as Projectile motion) will not support other object-level animation types (such as oscillation or ADSR, which is currently planned.). 

This document **only** outlines the instrument-level animation type "Evaluate". See [Animation Types](animation_types.md) for more details on other instrument-level animation types.
```


```{note}
Only location, Rotation (Euler), custom properties (floats and ints only) and shape key FCurves are supported currently.
```

To use location and rotation:
   - Keyframe your reference object just like you would any normal animation.
   - Select your target object, and in the Note On/Off object field, assign it to the reference object. 
   - Thats it!


To create custom properties and have them duplicated across all objects, first:
   - Create a custom property in the object tab on your reference object. 
   - Open the settings up and change the name to something meaningful. 
   - Change the default value to 0 (you might want to change the min/max's beyond their default values).
   - Press OK, and keyframe the property field with the hotkey I. Keyframe it like you would any other animation
   - Select your target object, and in the Note On/Off object field, assign it to the reference object. 
   - Thats it! Objects will then have the custom property duplicated to them once the script is executed. You can use this also do drive other parameters with Blender's Drivers.


To use shape keys for animation:
   - Create a shape key on the reference object. The object doesn't have to have the same deformations.
   - Name it the same name that the shape keys are on the target objects.
   - Next, keyframe the value field with hotkey I. Keyframe it like you would any other animation.
   - Select your target object, and in the Note On/Off object field, assign it to the reference object. 
   - Thats it! If the shape keys do not exist on the target objects, it will simply be ignored. If it finds a matching shape key name, it will be animated.  

```{admonition} Important
:class: danger
ALL animation must be relativie to 0. The first keyframe must start at 0, and the last keyframe must return to 0. If this is not followed, resulting animation will have side-effects.
```

```{note}
Only "Note On" is supported currently, which means you cannot create "sustaining" notes (but you can fake it for now!).
```

Note anchor points tell you when to trigger the animation based on the note on time. For example, if you had a note that came on at frame 30, and you want the animation to trigger on frame 25, you would set the note anchor point to -5 (start 5 frames before the note time). 

![cube_anim_note_anchor_pt](https://raw.githubusercontent.com/jamesa08/MIDIAnimatorDocs/main/docs/images/cube_anim_note_anchor_pt.png)
*Note anchor points are located next to the Note On/Off object field.*

There are differnet ways to deal with overlapping animation. Currently, the only way researched is to "add" the motion together. This works great for oscillating motion, but for other types of motion, it may give undesirable results. More overlapping methods will be added as they are discovered and researched.

<hr>

## Step 3: Writing the Code

```{note}
Currently there is no UI, and it may be some time before a UI is implemented, as there are several design challenges. 
```

Open up a Text Editor window. I also suggest you open up a [Console Window](https://docs.blender.org/manual/en/2.79/advanced/command_line/introduction.html) if you want to print messages to the screen.


Here is a sample script for analyzation:
```python
import bpy
from MIDIAnimator.src.animation import MIDIAnimatorNode
from MIDIAnimator.data_structures.midi import MIDIFile

file = MIDIFile("/path/to/midi/file.mid")
pianoTrack = file.findTrack("Steinway Grand Piano")

animator = MIDIAnimatorNode()
animator.addInstrument(instrumentType="evaluate", midiTrack=pianoTrack, objectCollection=bpy.data.collections['Cubes'])

# Animate the MIDI file
animator.animate()
```
<hr>

1. The first 3 lines are importing the necessarry classes and functions into Python.

`import bpy` imports the bpy module into Blender, Blender's API.<br>
`from MIDIAnimator.src.animation import MIDIAnimatorNode` for interacting with the Blender scene and animating the objects<br>
`from MIDIAnimator.data_structures.midi import MIDIFile` for reading MIDI files<br>

2. Next, we need to find where the MIDI file is stored on the computer.

### Windows File Directories

Type your directories with forward slashes ("/") rather than backslashes ("\\\\").<br>
`file = MIDIFile("C:/Users/path/to/file.mid")`


If you prefer to use backslashes, put the letter "r" in front of the directory string.<br>
For example: `file = MIDIFile(r"C:\Users\path\to\file.mid")`

or

Type your directories with double back slashes.<br>
For example: `file = MIDIFile("C:\\Users\\path\\to\\file.mid")`


### Mac/Linux File Directories

Type your directories like you normally would in a command-line enviroment.

Typical Mac directories: `file = MIDIFile("/Users/path/to/file.mid")`<br>
Typical Linux directories (varies by distro): `file = MIDIFile('/home/username/path/to/file.mid")`


```{tip}
Blender relative paths work! Just type `//` infront of the filename, and Blender will attempt to find the MIDI file in the same directory as the `.blend` file. Make sure to save your work first!
```


3. Now that you have the file, there are a number of functions you can perform to pull data out of the MIDI file.<br>
The most commonly used one by far is `findTrack()`, in which it takes the name of the MIDI track and returns the `MIDITrack` object (which you will assign to a variable).<br>
`pianoTrack = file.findTrack("Steinway Grand Piano")`


````{tip}
Unsure of what tracks you have in your MIDI file? Print them to the [Console Window](https://docs.blender.org/manual/en/2.79/advanced/command_line/introduction.html) with this short code snippet:
    
```python
file = MIDIFile("/path/to/midi/file.mid")
print(file.listTrackNames())
```

````


<hr>

4. Next, we need to setup a `MIDIAnimatorNode`. This will be the interface to tell MIDIAnimator what instruments to animate.


The first step in this process is to create a MIDIAnimatorNode() object and assign it to a variable:<br>
`animator = MIDIAnimatorNode()`


5. Next, we will add all of the instruments we want to animate. In the case of the Cubes collection, we will add it using the `addInstrument()` method:<br>
`animator.addInstrument(instrumentType="evaluate", midiTrack=pianoTrack, objectCollection=bpy.data.collections['Cubes'])`

```{note} 
See [Animation Types](animation_types.md) for more details on other instrument-level animation types.
```

6. Finally, we call the `animate()` method, which animates all instruments that were added to the `MIDIAnimatorNode` object.

<hr>

## Step 4: Running the Program
If all went well, you should see things moving to the notes of the MIDI file!

```{warning}
All objects in the collection passed in (for example, the "Cubes" collection) that have animation data will be cleared and overwitten.
```

```{note}
If you get stuck or run into issues, feel free to open an issue on the GitHub, or join the [Animusic Discord](https://discord.gg/yDfyhfA) and post in the #midianimator-discussion channel.
```


<br><hr>


## Writing Custom Animation Functions

```{note}
This section is under development. Check back later.
```

Sample code to write custom animations.


```python
import bpy
from MIDIAnimator.src.animation import MIDIAnimatorNode
from MIDIAnimator.src.instruments import Instrument
from MIDIAnimator.data_structures.midi import MIDIFile

class CustomInstrument(Instrument):
    def __init__(self, midiTrack: MIDITrack, collection: bpy.types.Collection, **kwargs):
        super().__init__(midiTrack, collection, override=True)
    
    def animate(self):
        # Write your custom animation here...
        pass

file = MIDIFile("/path/to/midi/file.mid")
pianoTrack = file.findTrack("Steinway Grand Piano")

animator = MIDIAnimatorNode()
animator.addInstrument(instrumentType="custom", midiTrack=pianoTrack, objectCollection=bpy.data.collections['Cubes'], custom=CustomInstrument)

# Animate the MIDI file
animator.animate()
```
