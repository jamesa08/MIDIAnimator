# Instrument Animation Types

## Evaluate
"evaluates" an animation, either via a object or a procedural function.


### Instrument level features:

*None*

### Object level features:
* Note input (both MIDI note (60) and Note Name (C3) supported)
* Individualized Note On/Off animation input
* Object animation types (Keyframed, Dampned Oscillation generator, ADSR envelope generator)
* Time mappers
* Amplitude mappers
* Velocity intensity slider
* Animation overlap handling (additive)

<hr>

## Projectile

Pre-defined animation code that emulates a projectile launching. Smart caching feautres to only use enough objects to animate all projectiles.

The collection that you assign this instrument type to is the "funnel" collection, or where you want the projectiles to initally start from. 

### Instrument level features:
* Projectile Collection (where the projectile objects get stored)
* Reference Projectile (the object that gets copied to create the projectiles)
* Use Inital Location (whether or not to use the initial location of the projectile curves)
* Use Angle Based Location:
    * This allows you to use external objects to determine the angle of the projectile.
    * Offset (in degrees)
    * Location Collection (where the angle objects get stored. Must also have proper note numbers for each cooresponding funnel)

### Object level features:
* Note input (both MIDI note (60) and Note Name (C3) supported)
* Projectile Curve (the reference curves that get copied to create the projectiles)
* Hit Time (where the ball hits the object. This will offset the animation in frames. - for earlier, + for later.)

<hr>

## Custom:

Defined by user, class and other parameters passed in via `addInstrument()`.
More details coming soon.

```{note}
More animation types will be added in the future.
```
