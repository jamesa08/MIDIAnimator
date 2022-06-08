from __future__ import annotations
import bpy
from contextlib import suppress
from dataclasses import dataclass
from typing import Callable, List, Tuple, Dict, Optional, Union

from . MIDIStructure import MIDIFile, MIDITrack, MIDINote
from .. utils.functions import noteToName
from .. utils.blender import FCurvesFromObject, delete_markers, secToFrames, cleanCollection, setInterpolationForLastKeyframe
from .. utils.algorithms import *

@dataclass
class BlenderObjectFrames:
    """
    this stores an object that will be moving from _startFrame to _endFrame
    """
    _startFrame: int
    _endFrame: int
    _instrument: Optional[GenericInstrument]  # some subclass of it

    def __post_init__(self):
        self._cachedObj: Optional[bpy.types.Object] = None
    
    # def dataForKeyframe(self, frame: int) -> Tuple[float, float, float]:
    #     """
    #     frame: frame number
    #     returns: x, y, z location of object for the specified frame (computed using the FCurve)
    #     """
    #     # calculate the location for the object for the specified frame using the FCurve
    #     # old code is this but it seems to only return two values
    #     # TODO: VERY TEMP
    #
    #     fCrv = FCurvesFromObject(self._instrument._obj.animation_curve)
    #     return fCrv[0].evaluate(frame), fCrv[1].evaluate(frame)

    def updateInstrument(self, instrument: BlenderAnimation):
        self._instrument = instrument

    def updateCachedObject(self, obj: bpy.types.Object) -> None:
        self._cachedObj = obj

    # so we can call sort() on a list of BlenderObjectFrames and have it sort them by start frame
    def __lt__(self, other: BlenderObjectFrames):
        return self._startFrame < other._startFrame

# @dataclass
class GenericInstrument:
    """base class for instruments that are played for notes"""
    _obj = bpy.types.Object

    def __init__(self, obj: bpy.types.Object) -> None:
        self._obj = obj

    # def __post_init__(self):
    #     # Use helper method to get self._obj.animaton_curve data & data_path parameters
    #     pass

    def processForNoteHitTime(self, timeOn: float) -> List[BlenderObjectFrames]:
        raise NotImplementedError("subclass must override")

    def positionForFrame(self, frameNumber: int) -> Tuple[float, float, float]:
        raise NotImplementedError("subclass must override")
        # TODO: new name: createKeyFrame?
        # instead of returning, just make the keyframe with keyframe_insert() and return None
        # eval FCurve
        # FIXME we will fix this with a way better solution soon
        # return FCurvesFromObject(self._obj.animation_curve)[0].evaluate(frameNumber), FCurvesFromObject(self._obj.animation_curve)[1].evaluate(frameNumber)

class FunnelInstrument(GenericInstrument):

    def processForNoteHitTime(self, timeOn: float) -> BlenderObjectFrames:
        # BlenderObjectFrames for the funnel/ball setting its blenderObjectType == "ball" (so can later get _blenderObject from the cache)
        hit = self._obj.note_hit_time
        
        rangeVector = FCurvesFromObject(self._obj.animation_curve)[self._obj.animation_curve_index].range()
        startFCurve, endFCurve = rangeVector[0], rangeVector[1]
        
        frame = secToFrames(timeOn)
        startFrame = round(int((startFCurve - hit) + frame))
        endFrame = round(int((endFCurve - hit) + frame))
        
        return BlenderObjectFrames(startFrame, endFrame, None)

    def positionForFrame(self, frameNumber: int) -> Tuple[float, float, float]:
        return FCurvesFromObject(self._obj.animation_curve)[0].evaluate(frameNumber), FCurvesFromObject(self._obj.animation_curve)[1].evaluate(frameNumber)


class StringInstrument(GenericInstrument):
    
    def processForNoteHitTime(self, timeOn: float) -> BlenderObjectFrames:
        # BlenderObjectFrames for the funnel/ball setting its blenderObjectType == "ball" (so can later get _blenderObject from the cache)
        
        rangeVector = FCurvesFromObject(self._obj.animation_curve)[self._obj.animation_curve_index].range()
        startFCurve, endFCurve = rangeVector[0], rangeVector[1]
        
        frame = secToFrames(timeOn)
        startFrame = round(int(startFCurve + frame))
        endFrame = round(int(endFCurve + frame))

        return BlenderObjectFrames(startFrame, endFrame, None)
    
    def positionForFrame(self, frameNumber: int) -> Tuple[float, float, float]:
        # eval FCurve
        # TODO: VERY TEMP
        return FCurvesFromObject(self._obj.animation_curve)[self._obj.animation_curve_index].evaluate(frameNumber)

@dataclass(init=False)
class CacheInstance:
    # data structure similar to a stack implementation
    _cache = List[bpy.types.Object]

    def __init__(self, objs: List[bpy.types.Object]) -> None:
        """intializes a type List[bpy.types.Object] to the cache instance."""
        self._cache = objs

    # def initializeObject(self, objType: str, objs: List[bpy.types.Object]) -> None:
    #     """intializes a type List[bpy.types.Object] to the cache instance. This *will* overwrite any existing data."""
    #     self._cache[objType] = objs

    def pushObject(self, obj: bpy.types.Object) -> None:
        """pushes a bpy.types.Object back to the cache. This is the method to use when you are done with the object"""
        # self._cache[objType].append(obj)
        self._cache.append(obj)
    
    def popObject(self) -> bpy.types.Object:
        """removes a bpy.types.Object from the cache and returns it. This is the method to use when you want to take an object out"""
        # obj = self._cache[objType].pop()
        obj = self._cache.pop()
        return obj


class BlenderTrack:
    _midiTrack: MIDITrack
    _blenderInstruments: Dict[int, GenericInstrument]  # key is the note's number
    _frames: List[BlenderObjectFrames]
    _cacheInstance: CacheInstance

    def __init__(self, midiTrack: MIDITrack, col: bpy.types.Collection):
        self._midiTrack = midiTrack
        self._blenderInstruments = {}
        
        # map obj notes to classes
        self._mapObjToCls(col)

        # compute the start & end times
        self._frames = self._makeFrames()

        self._cacheInstance = None

    def _mapObjToCls(self, col: bpy.types.Collection):
        for obj in col.all_objects:
            if obj.note_number is None: raise RuntimeError(f"Object '{obj.name}' has no note number!")
            if int(obj.note_number) in self._blenderInstruments: print(RuntimeWarning(f"Object '{obj.name}' has 2 notes."))

            insType = col.instrument_type

            if insType == "projectile":
                # obj is a funnel object
                if col.projectile_collection is None: raise ValueError("Projectile Collection for instrument type Projectile must be defined!")
                self._blenderInstruments[int(obj.note_number)] = FunnelInstrument(obj)
            
            elif insType == "string":
                self._blenderInstruments[int(obj.note_number)] = StringInstrument(obj)

    # make this a standalone method that returns the list rather than storing it since we don't need it after we're done
    # processing the track
    def _makeFrames(self) -> List[BlenderObjectFrames]:
        """
        computes the start & end frames for a BlenderTrack
        returns list of BlenderObjectFrames
        """
        
        out = []
        assert self._blenderInstruments is not None

        for note in self._midiTrack.notes:
            if self._blenderInstruments[note.noteNumber] is None:
                print(f"Note {note.noteNumber}/{noteToName(note.noteNumber)} in MIDITrack has no Blender object to map to!")
                continue  # soft warn, user could be wanting to ignore notes in track. However, it's good for debugging.
            
            instrument = self._blenderInstruments[note.noteNumber]
            frameInfo = instrument.processForNoteHitTime(note.timeOn)
            frameInfo.updateInstrument(instrument)
            # REFACTOR - this will now be out.extend and get the results of calling the frames method on the blenderInstrument for this note
            out.append(frameInfo)

        return out


class BlenderAnimation:
    """this class acts as a wrapper for the BlenderTracks class"""
    _blenderTracks: List[BlenderTrack]

    def __init__(self):
        self._blenderTracks = []

    def addInstrument(self, track: MIDITrack, objectCollection: bpy.types.Collection):
        """ make a BlenderTrack and add it into internal track list 
            cache only supported for type `projectile` instruments
        """
        assert isinstance(track, MIDITrack), "Please pass in a type MIDITrack object."
        assert isinstance(objectCollection, bpy.types.Collection), "Please pass in a type collection for the objects to be animated."
        
        blenderTrack = BlenderTrack(track, objectCollection)
        self._blenderTracks.append(blenderTrack)

        insType = objectCollection.instrument_type

        # pre animation operations
        if insType == "projectile":

            # delete old objects
            assert objectCollection.projectile_collection is not None, "Please define a Projectile collection for type Projectile."
            assert objectCollection.reference_projectile is not None, "Please define a reference object to be duplicated."
            cleanCollection(objectCollection.projectile_collection, objectCollection.reference_projectile)

            # calculate number of needed projectiles & instance the blender objects using bpy
            maxNumOfProjectiles = maxNeeded(blenderTrack._frames)  

            projectiles = []

            for i in range(maxNumOfProjectiles):
                duplicate = objectCollection.reference_projectile.copy()
                duplicate.name = f"{insType}_{hex(id(blenderTrack))}_{i}"
                objectCollection.projectile_collection.objects.link(duplicate)
                projectiles.append(duplicate)
            
            # create CacheInstance object
            blenderTrack._cacheInstance = CacheInstance(projectiles)

        elif objectCollection.instrument_type == "string":
            # TODO: clean keyframes on objects
            pass

    def animate(self) -> None:
        delete_markers("debug")

        # FIXME: delete all keyframes from frameStart, frameEnd = bpy.context.scene.frame_start, bpy.context.scene.frame_end

        for blenderTrack in self._blenderTracks:
            # copy the frames to not mutate them, then sort by start time and then reverse
            objectFrames = blenderTrack._frames[:]
            objectFrames.sort(reverse=True)


            activeObjectList: List[BlenderObjectFrames] = []
            
            # variables
            cache = blenderTrack._cacheInstance
            midiTrack = blenderTrack._midiTrack
            blenderInstruments = blenderTrack._blenderInstruments
            frameStart, frameEnd = bpy.context.scene.frame_start, bpy.context.scene.frame_end

            # XXX offeset frameStart by the first objectFrame's start time if it's negative
            # XXX discussion question, could frameStart, frameEnd be simplified by using this? we only need to animate the range between
            # XXX objectFrames[-1]._startFrame, objectFrames[0]._endFrame
            
            objFirstFrame, objLastFrame = objectFrames[-1]._startFrame, objectFrames[0]._endFrame
            # frameStart = objFirstFrame if objFirstFrame < 0 else frameStart
            frameStart, frameEnd = objFirstFrame - 1, objLastFrame + 1  # -1 & +1 to make sure they're within bounds



            # main loop
            for frame in range(frameStart, frameEnd):

                stillActiveList = []

                # delete/return to cache for old objects
                for frameInfo in activeObjectList:
                    objStartFrame = frameInfo._startFrame
                    objEndFrame = frameInfo._endFrame
                    cachedObj = frameInfo._cachedObj

                    if objEndFrame >= frame:
                        stillActiveList.append(frameInfo)
                    else:
                        # RETURN OBJECT TO CACHE
                        if cache is not None and cachedObj is not None:
                            cache.pushObject(cachedObj)
                            # FIXME make ball invisible
                
                activeObjectList = stillActiveList


                # determine which new objects to add
                i = len(objectFrames) - 1

                while i >=0 and objectFrames[i]._startFrame <= frame:
                    frameInfo = objectFrames[i]
                    objStartFrame = frameInfo._startFrame
                    objEndFrame = frameInfo._endFrame
                    cachedObj = frameInfo._cachedObj
                    blenderInstrument = frameInfo._instrument


                    if isinstance(blenderInstrument, FunnelInstrument):
                        if objStartFrame == frame:
                            cachedObj = cache.popObject()

                            # make last keyframe interpolation constant
                            setInterpolationForLastKeyframe(cachedObj, "CONSTANT")
                            # update cached object in frameIno
                            frameInfo._cachedObj = cachedObj

                            # FIXME add method to turn on object

                            activeObjectList.append(frameInfo)
                    elif isinstance(blenderInstrument, StringInstrument):
                        if objStartFrame == frame:
                            # Debug markers
                            bpy.context.scene.timeline_markers.new("debug", frame=frame)
                        
                        activeObjectList.append(frameInfo)
                    
                    # this note will be played next, so we shouldn't iterate over it again for the next frame
                    objectFrames.pop()
                    i -= 1

                # add a key frame for each object that is moving during this frame
                for frameInfo in activeObjectList:
                    objStartFrame = frameInfo._startFrame
                    objEndFrame = frameInfo._endFrame
                    blenderInstrument = frameInfo._instrument
                    obj = blenderInstrument._obj  # funnel  

                    if isinstance(blenderInstrument, FunnelInstrument):
                        cachedObj = frameInfo._cachedObj  # ball

                        hitTime = obj.note_hit_time  # get hit time from funnel
                        delta = frame - objStartFrame

                        x = obj.location[0]
                        y, z = blenderInstrument.positionForFrame(delta)

                        # make a keyframe for the object for this frame
                        cachedObj.location = (x, y, z)
                        cachedObj.keyframe_insert(data_path="location", frame=frame)
                        setInterpolationForLastKeyframe(cachedObj, "BEZIER")
                    
                    elif isinstance(blenderInstrument, StringInstrument):
                        delta = frame - objStartFrame
                        
                        obj.location[2] = blenderInstrument.positionForFrame(delta)
                        obj.keyframe_insert(data_path="location", index=2, frame=frame)
