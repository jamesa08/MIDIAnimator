from __future__ import annotations
import bpy
from typing import List, Tuple, TYPE_CHECKING
from math import sin, cos, pi, e
from .. data_structures.midi import MIDITrack
from .. data_structures import Keyframe

if TYPE_CHECKING:
    from ..data_structures import FrameRange

def maxSimultaneousObjects(intervals: List[FrameRange]) -> int:
    """
    :param intervals: List[FrameRange]
    :return int: max number of objects that are visible at any point in time
    """
    # keep track of maximum number of active items
    maxCount = 0

    # number of active items currently
    activeCount = 0
    # list of end times for currently active items sorted by the end time
    endTimesForActive = []

    # for testing code
    # currentActives = []

    # for each (start frame, end frame) interval for objects
    for frameRange in intervals:
        start = frameRange.startFrame
        end = frameRange.endFrame
        endIndex = 0
        endTimesCount = len(endTimesForActive)
        # remove active objects whose end time is before this new interval we are processing
        # while end times left to check and the end time is before the start time for this interval
        while endIndex < endTimesCount and endTimesForActive[endIndex] < start:

            # testing code to output intermediate steps
            # for (i, (x, y)) in enumerate(currentActives):
            #     if y == endTimesForActive[endIndex]:
            #         del currentActives[i]
            #         break

            # this one has ended so move to next index and decrease current active count
            endIndex += 1
            activeCount -= 1


        # remove all the ones from list whose end time is earlier than the start of this interval
        endTimesForActive = endTimesForActive[endIndex:]

        # testing code
        # currentActives.append((start, end))
        # print(start, end, currentActives)

        # add the item for this interval
        activeCount += 1
        # update maxCount if new maximum
        if activeCount > maxCount:
            maxCount = activeCount

        # put it in the list of endTimesForActive objects
        endTimesForActive.append(end)
        # position it in sorted order using insertionSort algorithm
        # this should be efficient since in general the new end times will usually go at end of list
        pos = len(endTimesForActive) - 1
        newItem = endTimesForActive[pos]
        while pos > 0 and endTimesForActive[pos - 1] > newItem:
            endTimesForActive[pos] = endTimesForActive[pos - 1]
            pos -= 1
        endTimesForActive[pos] = newItem

    # after processing all intervals return the computed maximum active count
    return maxCount

def animateSine(time: float, startVal: float, endVal: float, duration: float) -> float:
    # https://www.desmos.com/calculator/kw2grve25z
    time = min(max(time, 0), duration)  # clamps time 
    return -((cos((1 / duration) * pi * time) - 1) * ((endVal - startVal) / 2)) + startVal

def animateDampedOsc(x: float, period: float, amplitude: float, damp: float, frameRate: float):
    return (e ** -((damp * x) / frameRate)) * (-sin((period * x) / frameRate) * amplitude)

def genDampedOscKeyframes(period: float, amplitude: float, damp: float, frameRate: float) -> List[Keyframe]:
    # generates keyframes that will generate the specified dampened oscillation
    # Thanks to TheZacher5645 for helping figure out calculating the local extrema & derivative functions
    # ineractive demo: https://www.desmos.com/calculator/vzwitwmib6
    
    def waveFunc(x):
        return animateDampedOsc(x, period, amplitude, damp, frameRate)
    
    def firstDerivative(x):
        return (waveFunc(x) - waveFunc(x-0.001)) / 0.001
    
    def secondDerivative(x): 
        return (waveFunc(x) - (2 * waveFunc(x-0.001)) + waveFunc(x-(2*0.001))) / (0.001 ** 2)
    
    def newtonsMethod(x):
        if secondDerivative(x) == 0: return 0
        return x - (firstDerivative(x)/secondDerivative(x))
    
    y = 1
    i = 0
    
    out = [Keyframe(0, 0)]
    
    while abs(y) > 0.01:
        approxX = (i + 1/2) * (pi / period) * frameRate
        i += 1
        
        x = newtonsMethod(newtonsMethod(newtonsMethod(newtonsMethod(approxX))))
        if x < 0: continue
        y = waveFunc(x)
        
        out.append(Keyframe(x, waveFunc(x)))
    
    out.append(Keyframe(x + 1, 0))
    
    return out

def addKeyframes(insertedKeys: List[Keyframe], nextKeys: List[Keyframe]) -> None:
    """adds the two lists of keyframes together.
    check out the desmos graph to learn a bit more on how this works
    https://www.desmos.com/calculator/t7ullcvosp
    this is a mutating function
    
    :param List[Keyframe] insertedKeys: the keyframes that are already inserted on the object
    :param List[Keyframe] nextKeys: the keyframes that will be inserted next (the upcoming note)
    :raises ValueError: if the `insertedKeys'` first frame is bigger than `nextKeys'` first frame
    :return None: this function mutates the insertedKeys list
    """
    # for handling adding keyframes together
    def findOverlap(keyList1: List[Keyframe], keyList2: List[Keyframe]) -> List[Keyframe]:
        """finds the overlap between two sets of keylists 
        for this to work, the second keylist must be bigger than the first keylist

        :param List[Keyframe] keyList1: first keylist
        :param List[Keyframe] keyList2: second keylist
        :raises ValueError: if the first keylist's first frame is bigger than the second keylist's first frame
        :return List[Keyframe]: the list of keyframes that are overlapping
        """
        if len(keyList1) == 0 or len(keyList2) == 0:
            return []
        
        if keyList1[0].frame > keyList2[0].frame:
            # this means a note is somehow going back in time? is this even possible?
            # notes should always be sequential, and not in reverse time
            raise ValueError("first keyframe in keyList1 is bigger than first keyframe in keyList2! Please open a issue on GitHub along with the MIDI file.")
        
        overlappingKeyList = []
        overlapping = False
        for key1 in reversed(keyList1):
            if key1.frame > keyList2[0].frame:
                overlapping = True
                overlappingKeyList.append(key1)
            else:
                # not overlapping
                if overlapping:
                    overlappingKeyList.append(key1)
                break

        return list(reversed(overlappingKeyList))

    # for handling adding keyframes together
    def getValue(key1: Keyframe, key2: Keyframe, frame: float) -> float:
        """this interpolates 2 keyframes to get a intermediate value

        :param Keyframe key1: first keyframe
        :param Keyframe key2: second keyframe
        :param float frame: the frame to evaluate
        :return float: the evaluated value at the given frame
        """
        x1, y1 = key1.frame, key1.value
        x2, y2 = key2.frame, key2.value    
        try:
            m = (y2 - y1) / (x2 - x1)
        except ZeroDivisionError:
            # i dont know if this will work every time
            m = 0
        
        c = y1 - m * x1
        return (m * frame) + c

    # for handling adding keyframes together
    def interval(keyList, frame) -> Tuple[Keyframe]:
        """returns the interval keyframes back given a frame number
        e.g., if you had a keyList of 
        [
            Keyframe(frame=0.0, value=0.0), 
            Keyframe(frame=10.0, value=1.0), 
            Keyframe(frame=20.0, value=0.0)
        ]
        and you wanted to know the interval at frame 12.0
        the function would return
        (Keyframe(frame=10.0, value=1.0), Keyframe(frame=20.0, value=0.0))

        :param List[Keyframes] keyList: the list of keyframes to check
        :param float frame: the frame to check the interval between
        :return Tuple[Keyframe]: the keyframes that are within that interval
        """
        if len(keyList) == 0: 
            return (None, None)
        if keyList[0].frame > frame:
            # out of range to the left of the list
            return (keyList[0], keyList[0])
        elif keyList[-1].frame < frame:
            # out of range to the right of the list
            return (keyList[-1], keyList[-1])
        
        for i in range(len(keyList)):
            if keyList[i].frame <= frame <= keyList[i+1].frame:
                return (keyList[i], keyList[i+1])

    keysOverlapping = findOverlap(insertedKeys, nextKeys)

    insertedKeysInterValues = []
    nextKeysInterValues = []

    # interpolate the keyframes for each set of keyframes
    for key in nextKeys:
        inv1, inv2 = interval(keysOverlapping, key.frame)
        if inv1 is None and inv2 is None: continue
        nextKeysInterValues.append(Keyframe(key.frame, getValue(inv1, inv2, key.frame)))

    for key in keysOverlapping:
        inv1, inv2 = interval(nextKeys, key.frame)
        if inv1 is None and inv2 is None: continue
        insertedKeysInterValues.append(Keyframe(key.frame, getValue(inv1, inv2, key.frame)))

    # now add the keyframe values together (the most important part)
    for key, interp in zip(keysOverlapping, insertedKeysInterValues):
        key.value += interp.value

    for key, interp in zip(nextKeys, nextKeysInterValues):
        key.value += interp.value

    # extend the lists 
    # TODO (need a better method to ensure the keyframes before get cut off and then start )
    keysOverlapping.extend(nextKeys)
    keysOverlapping.sort(key=lambda keyframe: keyframe.frame)

    insertedKeys.extend(keysOverlapping)
