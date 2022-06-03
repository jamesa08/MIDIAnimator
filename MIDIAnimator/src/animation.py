from __future__ import annotations

from inspect import Attribute
from . MIDIStructure import MIDIFile, MIDITrack, MIDINote
from .. utils.functions import noteToName
from .. utils.animation import FCurvesFromObject, secToFrames
from .. utils.algorithms import *
from typing import Callable, List, Tuple, Dict, Optional
import bpy


class BlenderObjectFrames:
    """
    this stores an object that will be moving from _startFrame to _endFrame
    """
    _startFrame: int
    _endFrame: int
    _blenderObject: bpy.types.object
    _cacheType: Optional[str] # None if not a cached object otherwise this is used to determine which cache to put it back in
    # FIXME
    # need some way of determining what to do if we start animating this when already is animating (such as a string being hit while it is still vibrating)

    # may also need a reference back to the BlenderInstrument it goes with or at least type information
    # so that when it's a projectile we can return it to the cache
    # should we also have the FCurve (or list of FCurves) here or do we get them from the _blenderObject?

    def locationForFrame(self, frame: int) -> Tuple[float, float, float]:
        """
        frame: frame number
        returns: x, y, z location of object for the specified frame (computed using the FCurve)
        """
        # calculate the location for the object for the specified frame using the FCurve

        # old code is this but it seems to only return two values
        # eval FCurve
        # TODO: VERY TEMP
        # return ( FCurvesFromObject(self._blenderObject.animation_curve)[0].evaluate(frame),
        #          FCurvesFromObject(self._blenderObject.animation_curve)[1].evaluate(frame)

        pass

    # so we can call sort() on a list of BlenderObjectFrames and have it sort them by start frame
    def __lt__(self, other: BlenderObjectFrames):
        return self._startFrame < other._startFrame


class BlenderInstrument:
    """base class for instruments that are played for notes"""
    _noteNumber: int
    _classType: str

    _keyFrameInformation: List[BlenderObjectFrames]

    def __init__(self, noteNumber: int, classType: str):
        self._noteNumber = noteNumber
        self._classType = classType

    def processForNoteHitTime(self, timeOn: float):
        # set _keyFrameInformation
        raise NotImplementedError("subclass must override")

    def frames(self) -> List[BlenderObjectFrames]:
        return self._keyFrameInformation

# maybe have a FunnelPlayedInstrument class that FunnelStringInstrument and FunnelCymbalInstrument subclass since
# they are similar and may be able to share some code in the base the FunnelPlayedInstrument class

class FunnelStringInstrument(BlenderInstrument):

    _funnel: bpy.types.object
    _string: bpy.types.object
    _projectile: bpy.types.object # the ball that we get from the cache

    # do we pass in the funnel and string bpy.types.object or does this class use Blender API to get via the metadata?
    def __init__(self, noteNumber: int, classType: str):
        super().__init__(noteNumber, classType)


    def processForNoteHitTime(self, timeOn: float):
        pass
        # will put two items in the list _keyFrameInformation
        #     BlenderObjectFrames for the funnel/ball
        #     BlenderObjectFrames for the string (using noteOn - maybe velocity to figure out how much string should move)

        # here is old code we were using to determine the frames for the ball - will use this idea

        # the FCurve stores offset at which the note is hit (metadata)
        # so the startFrame is note.timeOn minus that offset
        # calculate the endFrame based on the last controlPoint and the startFrame

        # hit = self._blenderObject.note_hit_time
        #
        # rangeVector = self._FCurve.range()
        # startFCurve, endFCurve = rangeVector[0], rangeVector[1]
        #
        # frame = secToFrames(timeOn)
        # self._startFrame = round(int(startFCurve - hit) + frame))
        # self._endFrame = round(int((endFCurve - hit) + frame))



# FIXME: to be removed
class AnimatableBlenderObject:
    classType = "generic"
    _blenderObject: bpy.types.Object
    _note: int
    # _projectile: bpy.types.Object
    _projectileType: str
    _startFrame: int
    _endFrame: int
    _controlPoints: Tuple[float, float, float]

    def __init__(self, obj: bpy.types.Object, projectileType: str = ""):
        if obj.note_number is None: raise RuntimeError(f"Object {obj} has no note number!")
        if obj.animation_curve is None: raise RuntimeError(f"Object {obj} has no animation curve!")
        
        # determine _blenderObject using bpy and the note
        self._blenderObject = obj
        self._note = int(obj.note_number)
        self._FCurve = FCurvesFromObject(obj.animation_curve)[obj.animation_curve_index]
        self._projectileType = projectileType

        self._startFrame = None
        self._endFrame = None

    def calculateDataForNoteHitTime(self, timeOn: float):
        pass

    def startFrame(self) -> int:
        return self._startFrame

    def endFrame(self) -> int:
        return self._endFrame

    def _controlPoints(self):
        pass

    def positionForFrame(self, frameNumber: int) -> Tuple[float, float, float]:
        # eval FCurve
        # TODO: VERY TEMP
        return FCurvesFromObject(self._blenderObject.animation_curve)[0].evaluate(frameNumber), FCurvesFromObject(self._blenderObject.animation_curve)[1].evaluate(frameNumber)

# FIXME: to be removed
class BlenderObjectProjectile(AnimatableBlenderObject):

    classType = "projectile"
    _projectile: Optional[bpy.types.Object]
    
    def __init__(self, obj: bpy.types.Object, projectileType: str = ""):
        super().__init__(obj, projectileType)
        self._projectile = None

    def calculateDataForNoteHitTime(self, timeOn: float):
        # calculate other instance variables - startFrame, endFrame based on FCurve
        # for example if the note is to be played at frame 100

        # the FCurve stores offset at which the note is hit (metadata)
        # so the startFrame is note.timeOn minus that offset
        # calculate the endFrame based on the last controlPoint and the startFrame

        hit = self._blenderObject.note_hit_time
        rangeVector = self._FCurve.range()
        startFCurve, endFCurve = rangeVector[0], rangeVector[1]
        
        frame = secToFrames(timeOn)
        self._startFrame = int(round((startFCurve - hit) + frame))
        self._endFrame = int(round((endFCurve - hit) + frame))


    # def _getReusableProjectile(self, cache: ProjectileCache):
    #     pass

# FIXME: to be removed
class BlenderObjectString(AnimatableBlenderObject):
    
    classType = "string"

    def calculateDataForNoteHitTime(self, timeOn: float):
        # calculate other instance variables - startFrame, endFrame based on FCurve
        # for example if the note is to be played at frame 100

        # the FCurve stores offset at which the note is hit (metadata)
        # so the startFrame is note.timeOn minus that offset
        # calculate the endFrame based on the last controlPoint and the startFrame

        hit = self._blenderObject.note_hit_time

        rangeVector = self._FCurve.range()
        startFCurve, endFCurve = rangeVector[0], rangeVector[1]
        
        frame = secToFrames(timeOn)
        self._startFrame = (startFCurve - hit) + frame
        self._endFrame = (endFCurve - hit) + frame

# FIXME: to be removed
class BlenderObject:
    _obj: bpy.types.Object
    _animator: AnimatableBlenderObject

    def __init__(self, obj: bpy.types.Object, animator: AnimatableBlenderObject) -> None:
        self._obj = obj
        self._animator = animator

class ProjectileCache:
    # FIXME type annotations !!!!

    _cache: Dict[str, List[bpy.types.Object]]

    def __init__(self):
        self._cache = {}

    def addProjectileType(self, projectileType: str, projectiles: List[BlenderObject]):
        self._cache[projectileType] = projectiles

    def reusableProjectileForType(self, projectileType: str) -> BlenderObject:
        obj = self._cache[projectileType].pop()
        return obj
            # print("requested more projectiles than exist for this type: maxNumber was incorrect", e)

    def returnProjectileToCache(self, projectileType: str, blenderObject: BlenderObject):
        self._cache[projectileType].append(blenderObject)


class BlenderTrack:
    
    _midiTrack: MIDITrack
    _noteToBlender: Dict[int, Tuple[bpy.types.Object, AnimatableBlenderObject]]  # key is the note's number

    def __init__(self, midiTrack: MIDITrack):
        self._midiTrack = midiTrack
        self._noteToBlender = dict()
        self.attribute = "location"

    def setInstrument(self, col: bpy.types.Collection, projectileCollection: bpy.types.Collection):
        # iterate over objectCollection and get the note_numbers and FCurve data
        # build up a dictionary with the note as the key and the value will be a new AnimatableBlenderObject() or a sub-
        # class depending on the instrumentType parameter

        # in the blender object metadata, the funnel will need a way to get the corresponding string that the ball will be
        # launched and hit


        # REFACTOR: map each note to a subclass of BlenderInstrument depending on the instrument type - for now start with FunnelString
        for obj in col.all_objects:
            if obj.note_number is None: raise RuntimeError(f"Object {obj} has no note number!")

            # vals = (obj, obj.note_number, obj.animation_curve[obj.animation_curve_index])
            if col.instrument_type == "string":
                cls = BlenderObjectString(obj)
            elif col.instrument_type == "projectile":
                if projectileCollection is None: raise ValueError("Projectile Collection for instrument type Projectile is not passed!")
                cls = BlenderObjectProjectile(obj, col.instrument_type)
            
            vals = (obj, cls)
            
            self._noteToBlender[obj.note_number] = vals
            
    
    def computeStartEndFramesForObjects(self) -> List[Tuple[int, int, AnimatableBlenderObject]]:
        """
        returns: list of tuples (startFrame, endFrame, AnimatableBlenderObject)
        """
        #    make empty list
        #    for each note in sorted order (track._notesToPlay)  # already sorted
        #        animatableObject = use track dictionary that gets it for the note (using the note number as the key)
        #        animatableObject.calculateDataForNoteHitTime(note.timeOn)
        #        add tuple (animatableObject.startFrame(), .endFrame, animatableObject) to list
        #     return list
        
        out = []
        
        assert self._noteToBlender is not None

        for note in self._midiTrack.notes:
            assert self._noteToBlender[str(note.noteNumber)] is not None, f"Note {note.noteNumber}/{noteToName(note.noteNumber)} in MIDITrack has no Blender object to map to!"

            # REFACTOR why is str(note.noteNumber) used here - aren't we using the int value for the note as the key
            # we will now get back a BlenderInstrument subclass
            # FIXME key will be an int (note.noteNumber)
            obj, cls = self._noteToBlender[str(note.noteNumber)]
            cls.calculateDataForNoteHitTime(note.timeOn)
            # REFACTOR - this will now be out.extend and get the results of calling the frames method on the blenderInstrument for this note
            out.append((cls.startFrame(), cls.endFrame(), cls))


        # FIX return type annotation now a list of BlenderObjectFrames
        return out


# these are the objects you would instance in
class BlenderAnimation:
    _blenderTracks: List[BlenderTrack]
    _projectileCache: ProjectileCache

    def __init__(self):
        # call MIDIFile and read in  # if we want the user to be able to access the tracks & pass them in, then they should instance MIDIFile themselves
        self._blenderTracks = []
        self._projectileCache = ProjectileCache()

    def setInstrumentForTrack(self, track: MIDITrack, objectCollection: bpy.types.Collection, projectileCollection: bpy.types.Collection=None, attribute: str=None, ballName: str=None):
        # make a new BlenderTrack and add to _blenderTracks

        # get all notes for track   # .setInstrument() does that
        # create BlenderObjectProjectile()  # .setInstrument() does that
        # map each note to object and make AnimatableBlenderObject for each note (dict?)  # .setInstrument() does that
        
        blenderTrack = BlenderTrack(track)
        blenderTrack.setInstrument(objectCollection, projectileCollection)
        if attribute:
            blenderTrack.attribute = attribute
        
        self._blenderTracks.append(blenderTrack)

        insType = objectCollection.instrument_type

        # maybe these names should be funnelString, funnelCymbal
        if insType == "projectile":
            assert ballName is not None
            assert bpy.data.objects[ballName]

            self._cleanCollection(projectileCollection, ballName)  # TODO: remove hardcode "MAIN", let user define object to clone (new bpy.prop?)
            
            # calculate number of needed projectiles & instance the blender objects using bpy
            maxNumOfProjectiles = maxNeeded(blenderTrack.computeStartEndFramesForObjects())

            projectileType = bpy.data.objects[ballName].projectile_id

            projectiles = []

            for i in range(maxNumOfProjectiles):
                dup = bpy.data.objects[ballName].copy()
                dup.name = f"projectile_{i}"
                projectileCollection.objects.link(dup)
                projectiles.append(dup)

            self._projectileCache.addProjectileType(projectileType, projectiles)

            # TEMPORAY CODE 
            # need to figure out the max amount of balls visible in any frame
            # to make some balls for testing (make 1 ball per note)


            # for key in blenderTrack._noteToBlender:  # key == note number
            #     obj, cls = blenderTrack._noteToBlender[key]
            #     if insType == "projectile":
            #         dup = bpy.data.objects[ballName].copy()
            #         dup.name = f"{hex(id(cls))}_{key}"
            #         projectileCollection.objects.link(dup)
                    
                
                
    def _cleanCollection(self, col: bpy.types.Collection, name: str) -> None:
        """
        Cleans a collection of old objects (to be reanimated)
        """
        assert len(col.all_objects) != 0, "Please have a projectile in the collection to clone."

        objsToRemove = []
        found = False

        for obj in col.all_objects:
            if obj.name == name: 
                # keep the object
                found = True  
            else:
                objsToRemove.append(obj) 
        
        for obj in objsToRemove:
            bpy.data.objects.remove(obj, do_unlink=True)

        assert found is True, "Please provide a projectile in the collection to have cloned"
        assert len(col.all_objects) == 1

    # FIXME: delete old animate method when done with this and rename it animate
    def newAnimate(self) -> None:
        for track in self._blenderTracks:
            objectFrames = track.computeStartEndFramesForObjects()
            # sort by start time and then reverse
            objectFrames.sort(reverse=True)

            frameStart, frameEnd = bpy.context.scene.frame_start, bpy.context.scene.frame_end

            activeObjectList: List[BlenderObjectFrames] = []

            # FIXME: make empty set of the objects that can be hit while already moving (strings, cymbals, etc.)

            for frame in range(frameStart, frameEnd):

                # determine which objects are no longer active
                # maybe put this in a helper method
                stillActiveList = []
                for frameInfo in activeObjectList:
                    # if the object is still be animated include it in our list
                    if frameInfo._endFrame >= frame:
                        stillActiveList.append(frameInfo)
                        # if this is something that can be hit while already moving
                            # remove from the set of those objects
                    # we won't put it in the updated list
                    else:
                        pass
                        # if this is a ball/projectile return it to the cache
                        # use _frameInfo._cacheType (if not None call return to cache method with that value)
                activeObjectList = stillActiveList


                # determine which new objects to add
                i = len(objectFrames)
                while i >=0 and objectFrames[i]._startFrame <= frame:
                    pass

                    # determine the objects that will be moving
                    # put it in the activeObjectList
                    # for some instrument types, will need to get projectile from cache

                    # if it is an object that can be hit while already moving (like a string)
                        # if this object is in our set of these objects that are currently moving
                            # then recalculate the path for it (see AnimationNodes)
                            # will also need to update its endFrame
                        # else
                            # put in the set of objects that are currently

                # add a key frame for each object that is moving during this frame
                for frameInfo in activeObjectList:
                    location = frameInfo.locationForFrame(frame)
                    # make a keyframe for the object for this frame




    def animate(self) -> None:

        # create an empty list (combined list)
        # for each blender track
        #    data = track.computeStartEndFramesForObjects
        #    data is a list [(startFrame, endFrame, AnimatableBlenderObject), ...]
        #
        #  [2, 6, 8, 9] [1, 3, 5, 10, 11]

        # merge this list into combined list

        # NOTE: this is the _most_ efficient way to do this. I used timeit and different algorithms to test with, see: https://pastebin.com/raw/kab98YWS
        combined = [extendResult for track in self._blenderTracks for extendResult in track.computeStartEndFramesForObjects()]

        print(len(combined), combined)
        # sort by startFrame
        combined.sort(key=lambda tup: tup[0])
        combined.reverse()

        activeObjectList = []

        frameStart, frameEnd = bpy.context.scene.frame_start, bpy.context.scene.frame_end

        # activeObjectList: (startFrame, endFrame, BlenderObject)
        # combined: (startFrame, endFrame, AnimatableBlenderObject)
        for frame in range(frameStart, frameEnd):
            # del objects not used only if list is not empty (otherwise will fail)

            # remove from list of active objects and objects whose end frame is before this frame
            # activeObjectList = list(filter(lambda data: round(data[1]) >= frame, activeObjectList))
            updatedList = []
            for frameOn, frameOff, bObject in activeObjectList:
                abo = bObject._animator
                # projectile: BlenderObject()
                if frameOff >= frame:
                    updatedList.append((frameOn,frameOff,bObject))
                else:
                    print("returned object to cache")
                    self._projectileCache.returnProjectileToCache(bObject._obj.projectile_id, bObject._obj)
            
            activeObjectList = updatedList
            

            # update the list of active objects (add any new objects from combined list whose startFrame is this frame)
            # for efficiency reasons, the list is reversed, so we start at the end
            i = len(combined) - 1

            # while we still have notes to play and the start time for this note is on or before this frame, then add it to activeObjectList
            while i >= 0 and combined[i][0] <= frame:
                frameOn = combined[i][0]
                frameOff = combined[i][1]
                abo = combined[i][2]  # AnimatableBlenderObject

                if abo.classType == "projectile":
                    # animateProjectile(*args, self._projectileCache)

                    # fill in x from object
                    # get an object that is not being used (somehow)
                    projectileType = abo._blenderObject.projectile_id
                    print(projectileType)
                    print("requested object from cache")
                    if frameOn == frame:
                        obj = self._projectileCache.reusableProjectileForType("sphere")  # bpy.types.Object
                        # make last keyframe interpolation constant
                        try:
                            if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
                                for fCrv in FCurvesFromObject(obj):
                                    fCrv.keyframe_points[-1].interpolation = "CONSTANT"
                        except AttributeError:
                            pass

                        bObject = BlenderObject(obj, abo)
                        # FIXME I dont think we need _projectile in the AnimatableBlenderObject class
                        abo._projectile = obj
                        # x = abo._blenderObject.location[0]  # get the start pos from the funnel object
                        # y, z = abo.positionForFrame(0)
                    
                        activeObjectList.append((frameOn, frameOff, bObject))

                # delete this note b/c we added it to active object list & move to next note 
                combined.pop()
                i -= 1
            
            
            # newlyInsertedObjects = []

            for frameOn, frameOff, bObject in activeObjectList:
                obj = bObject._obj
                abo = bObject._animator

                hitTime = abo._blenderObject.note_hit_time  # get hit time from funnel
                delta = frame - frameOn

                x = abo._blenderObject.location[0]
                y, z = abo.positionForFrame(delta)

                obj.location = (x, y, z)
                obj.keyframe_insert(data_path="location", frame=frame)

                # FIXME: check if there is a less awkward way 
                try:
                    if obj is not None and obj.animation_data is not None and obj.animation_data.action is not None:
                        for fCrv in FCurvesFromObject(obj):
                            fCrv.keyframe_points[-1].interpolation = "BEZIER"
                except AttributeError:
                    pass


                # if bObject.classType == "projectile":
                #     # animateProjectile(*args, self._projectileCache)

                #     # fill in x from object
                #     # get an object that is not being used (somehow)
                #     projectileType = bObject._blenderObject.projectile_id
                #     print(projectileType)
                #     print("requested object from cache")
                #     if round(frameOn) == frame:
                #         obj = self._projectileCache.reusableProjectileForType("sphere")
                #         bObject._projectile = obj
                #         x = bObject._blenderObject.location[0]
                #         y, z = bObject.positionForFrame(0)

                #         newlyInsertedObjects.append((frameOn, frameOff, bObject))
                #     else:
                #         obj = bObject._projectile
                #         # objName = f"{hex(id(cls))}_{cls._note}"  # TEMP

                #         hitTime = bObject._blenderObject.note_hit_time
                #         delta = frame - frameOn

                #         x = bObject._blenderObject.location[0]
                #         y, z = bObject.positionForFrame(delta)

                #     obj.location = (x, y, z)
                #     obj.keyframe_insert(data_path="location", frame=frame)

            # activeObjectList.extend(newlyInsertedObjects)