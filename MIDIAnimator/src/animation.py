from __future__ import annotations
import bpy
from contextlib import suppress
from dataclasses import dataclass
from typing import Callable, List, Tuple, Dict, Optional, Union

from .. utils.algorithms import *
from .. utils.functions import noteToName
from . MIDIStructure import MIDIFile, MIDITrack, MIDINote
from .. utils.blender import FCurvesFromObject, deleteMarkers, secToFrames, cleanCollection, setInterpolationForLastKeyframe

class BlenderAnimation:
    """this class acts as a wrapper for GenericTracks/custom tracks"""
    _tracks: List[GenericInstrument]

    def __init__(self):
        self._tracks = []
        self._activeNoteDict = dict()

    def addInstrument(self, midiTrack: MIDITrack, objectCollection: bpy.types.Collection, custom=None, customVars: Dict=dict()):
        """ make a GenericInstrumnet subclass and add it into internal track list 
            cache only supported for type `projectile` instruments
        """
        assert isinstance(midiTrack, MIDITrack), "Please pass in a type MIDITrack object."
        assert isinstance(objectCollection, bpy.types.Collection), "Please pass in a type collection for the objects to be animated."
        
        insType = objectCollection.instrument_type
        
        if insType == "projectile": cls = ProjectileInstrument(midiTrack, objectCollection)
        elif insType == "string": cls = StringInstrument(midiTrack, objectCollection)
        elif insType == "custom":
            if custom is None: raise RuntimeError("Please pass a custom class object. Refer to the docs.")
            if customVars is not None: 
                cls = custom(midiTrack, objectCollection, **customVars)
            else:
                cls = custom(midiTrack, objectCollection)
        
        self._tracks.append(cls)

        
    def updateActiveObjectList(self, cls: GenericInstrument, objFrameRanges: List[FrameRange], activeObjectList: List[FrameRange], frame: int):
        # variables
        cache = None
        if hasattr(cls, '_cacheInstance'):
            cache = cls._cacheInstance

        stillActiveList = []
        # delete/return to cache for old objects
        for frameInfo in activeObjectList:
            objStartFrame = frameInfo.startFrame
            objEndFrame = frameInfo.endFrame
            obj = frameInfo.obj
            cachedObj = frameInfo.cachedObj

            if objEndFrame >= frame:
                stillActiveList.append(frameInfo)
            else:
                # this note is no longer being played
                # remove from activeNoteDict
                self._activeNoteDict[obj.note_number].remove(frameInfo)

                # RETURN OBJECT TO CACHE
                if cache is not None and cachedObj is not None:
                    cache.pushObject(cachedObj)

                    # disable cached object in viewport/render
                    cachedObj.hide_viewport = True
                    cachedObj.hide_render = True
                    cachedObj.keyframe_insert(data_path="hide_viewport", frame=frame)
                    cachedObj.keyframe_insert(data_path="hide_render", frame=frame)
        
        activeObjectList = stillActiveList


        # update activeObjectList with new objects
        i = len(objFrameRanges) - 1

        while i >=0 and objFrameRanges[i].startFrame <= frame:
            frameInfo = objFrameRanges[i]
            objStartFrame = frameInfo.startFrame
            objEndFrame = frameInfo.endFrame
            cachedObj = frameInfo.cachedObj
            obj = frameInfo.obj
            insType = cls.collection.instrument_type

            if cache is not None:
                # this is of cache type, 
                if objStartFrame == frame:
                    # GET OBJECT FROM CACHE
                    cachedObj = cache.popObject()

                    # make last keyframe interpolation constant
                    setInterpolationForLastKeyframe(cachedObj, "CONSTANT")
                    
                    # update cached object in frameIno
                    frameInfo.cachedObj = cachedObj

                    # enable cached object in viewport/render 
                    cachedObj.hide_viewport = False
                    cachedObj.hide_render = False
                    cachedObj.keyframe_insert(data_path="hide_viewport", frame=frame)
                    cachedObj.keyframe_insert(data_path="hide_render", frame=frame)

                    activeObjectList.append(frameInfo)
            else:
                # if not of cache type, just append the active frames
                activeObjectList.append(frameInfo)
                # if objStartFrame == frame:
                #     # Debug markers
                #     bpy.context.scene.timeline_markers.new("debug", frame=frame)
            
            # add note to activeNoteDict
            if obj.note_number in self._activeNoteDict:
                self._activeNoteDict[obj.note_number].append(objFrameRanges[i])
            else:
                self._activeNoteDict[obj.note_number] = [objFrameRanges[i]]

            # this note will be played next, so we shouldn't iterate over it again for the next frame
            objFrameRanges.pop()
            i -= 1
        
        return activeObjectList

    def animate(self) -> None:
        # deleteMarkers("debug")

        # FIXME: delete all keyframes from frameStart, frameEnd = bpy.context.scene.frame_start, bpy.context.scene.frame_end

        for track in self._tracks:
            if track.override == True:
                track.animate()
                continue
            
            # copy the frames to not mutate them, then sort by start time and then reverse
            objFrameRanges = track.createFrameRanges()
            objFrameRanges.sort(reverse=True)

            activeObjectList: List[FrameRange] = []
            
            # variables            
            frameStart, frameEnd = bpy.context.scene.frame_start, bpy.context.scene.frame_end

            # XXX offeset frameStart by the first objectFrame's start time if it's negative
            # XXX discussion question, could frameStart, frameEnd be simplified by using this? we only need to animate the range between
            # XXX objectFrames[-1]._startFrame, objectFrames[0]._endFrame
            
            objFirstFrame, objLastFrame = objFrameRanges[-1].startFrame, objFrameRanges[0].endFrame
            # frameStart = objFirstFrame if objFirstFrame < 0 else frameStart
            frameStart, frameEnd = objFirstFrame - 1, objLastFrame + 1  # -1 & +1 to make sure they're within bounds

            # clear out activeNoteDict
            self._activeNoteDict = dict()

            # main loop
            for frame in range(frameStart, frameEnd):
                activeObjectList = self.updateActiveObjectList(track, objFrameRanges, activeObjectList, frame)
                
                # update activeNoteDict: the notes that are still playing

                # add a keyframe for each object that is moving during this frame
                # call animate method to insert a keyframe
                # instead of passing activeObjectList, pass in activeNoteDict
                track.animate(self._activeNoteDict, frame)


