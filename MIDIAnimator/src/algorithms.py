from __future__ import annotations
import bpy
from typing import List, Tuple, TYPE_CHECKING
from math import sin, cos, pi, e, atan
from .. data_structures.midi import MIDITrack
from .. data_structures import Keyframe

if TYPE_CHECKING:
    from ..data_structures import FrameRange

def maxSimultaneousObjects(intervals: List[Tuple[float, float]]) -> int:
    """
    gets the max simotaneous objects for List[Tuple[float, float]]`.
    :param intervals: List[Tuple[float, float]]
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
    for start, end in intervals:
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
    """evaluates a sine curve based on paramters
    interactive demo: https://www.desmos.com/calculator/kw2grve25z

    :param float time: the time at which you want to evaluate from (e.g., f(x))
    :param float startVal: starting value (e.g., y-axis)
    :param float endVal: ending value (e.g., y-axis)
    :param float duration: how long the animation should last (e.g., x-axis)
    :return float: the result of the evaluated animation (e.g., result of evaluating f(x))
    """
    time = min(max(time, 0), duration)  # clamps time 
    return -((cos((1 / duration) * pi * time) - 1) * ((endVal - startVal) / 2)) + startVal

def animateDampedOsc(time: float, period: float, amplitude: float, damp: float):
    """evaluates a dampaned oscillation curve based on parameters

    :param float time: the time at which you want to evaluate from (e.g., f(x))
    :param float period: how long it takes for the oscillation to repeat one time
    :param float amplitude: how large is each oscillation
    :param float damp: damping of the oscillation (how much decrease of energy for each oscillation) 
    :param float frameRate: the framerate of the current scene
    :return _type_: _description_
    """
    return (e ** -((damp * time))) * (-sin((period * time)) * amplitude)

def genDampedOscKeyframes(period: float, amplitude: float, damp: float, frame=0) -> List[Keyframe]:
    """generates keyframes that will generate the specified dampend oscillation
    Thanks to TheZacher5645 for helping figure out calculating the local extrema & derivative functions for v1
    interactive demo (v2): https://www.desmos.com/calculator/qwmf2xkno3
    
    :param float period: how long it takes for the oscillation to repeat one time
    :param float amplitude: how large is each oscillation
    :param float damp: damping of the oscillation (how much decrease of energy for each oscillation) 
    :param float frameRate: the framerate of the current scene
    :return List[Keyframe]: a list of keyframes of the min's and max's of the oscillation. 
    """

    def waveFunc(x):
        return animateDampedOsc(x, period, amplitude, damp)

    def localExtrema(x):
        # these are the x intercepts of the first derivative of the wave function
        # in turn will give us our local extrema for the main wave function
        return (atan(period/damp)/period) + (x*pi/period)
    
    y = 1
    i = 0
    
    out = [Keyframe(frame, 0)]
    
    # negate period to positive and invert the amplitude if period is negative
    if period < 0:
        period = -period
        amplitude = -amplitude

    while abs(y) > 0.001:
        
        x = localExtrema(i)

        if x < 0: continue
        y = waveFunc(x)
        
        out.append(Keyframe(x+frame, waveFunc(x)))
        i += 1
    
    out.append(Keyframe(x + frame + 1, 0))
    
    return out

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
    
    for i in range(len(keyList) - 1):
        if keyList[i].frame <= frame <= keyList[i+1].frame:
            return (keyList[i], keyList[i+1])

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


def minKeyframes(insertedKeys: List[Keyframe], nextKeys: List[Keyframe]) -> None:
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

    # most important part: do the thing
    for key, interp in zip(keysOverlapping, insertedKeysInterValues):
        # check if the value is less than the interpolated value
        if key.value < interp.value:
            key.value = interp.value

    for key, interp in zip(nextKeys, nextKeysInterValues):
        # check if the value is less than the interpolated value
        if key.value < interp.value:
            key.value = interp.value

    # extend the lists 
    # TODO (need a better method to ensure the keyframes before get cut off and then start )
    keysOverlapping.extend(nextKeys)
    keysOverlapping.sort(key=lambda keyframe: keyframe.frame)

    insertedKeys.extend(keysOverlapping)


def maxKeyframes(insertedKeys: List[Keyframe], nextKeys: List[Keyframe]) -> None:
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

    # most important part: do the thing
    for key, interp in zip(keysOverlapping, insertedKeysInterValues):
        # check if the value is more than the interpolated value
        if key.value > interp.value:
            key.value = interp.value

    for key, interp in zip(nextKeys, nextKeysInterValues):
        # check if the value is more than the interpolated value
        if key.value > interp.value:
            key.value = interp.value

    # extend the lists 
    # TODO (need a better method to ensure the keyframes before get cut off and then start )
    keysOverlapping.extend(nextKeys)
    keysOverlapping.sort(key=lambda keyframe: keyframe.frame)

    insertedKeys.extend(keysOverlapping)

def prevKeyframes(insertedKeys: List[Keyframe], nextKeys: List[Keyframe]) -> None:
    keysOverlapping = findOverlap(insertedKeys, nextKeys)

    # maintain the value from the first set during overlap
    for key in keysOverlapping:
        for nextKey in nextKeys:
            if key.frame == nextKey.frame:
                nextKey.value = key.value

    # append non-overlapping keyframes from nextKeys to insertedKeys
    nonOverlappingNextKeys = [key for key in nextKeys if key.frame not in [k.frame for k in keysOverlapping]]
    insertedKeys.extend(nonOverlappingNextKeys)
    insertedKeys.sort(key=lambda keyframe: keyframe.frame)


def nextKeyframes(insertedKeys: List[Keyframe], nextKeys: List[Keyframe]) -> None:
    # Find overlapping keyframes between insertedKeys and nextKeys
    keysOverlapping = findOverlap(insertedKeys, nextKeys)

    # Replace values from insertedKeys with nextKeys during overlap
    for key in keysOverlapping:
        for nextKey in nextKeys:
            if key.frame == nextKey.frame:
                key.value = nextKey.value

    # Identify keyframes in nextKeys that are not in the overlap
    nonOverlappingNextKeys = [key for key in nextKeys if key.frame not in [k.frame for k in keysOverlapping]]

    # Combine non-overlapping nextKeys with all the original insertedKeys
    finalKeyframes = []
    i, j = 0, 0
    while i < len(insertedKeys) and j < len(nonOverlappingNextKeys):
        if insertedKeys[i].frame < nonOverlappingNextKeys[j].frame:
            finalKeyframes.append(insertedKeys[i])
            i += 1
        else:
            finalKeyframes.append(nonOverlappingNextKeys[j])
            j += 1

    # Add remaining keyframes
    finalKeyframes.extend(insertedKeys[i:])
    finalKeyframes.extend(nonOverlappingNextKeys[j:])

    # Clear the insertedKeys list and replace it with finalKeyframes
    insertedKeys.clear()
    insertedKeys.extend(finalKeyframes)


def restValueCrossingKeyframes(insertedKeys: List[Keyframe], nextKeys: List[Keyframe]) -> None:
    restValue = 0
    keysOverlapping = findOverlap(insertedKeys, nextKeys)

    for key in keysOverlapping:
        for nextKey in nextKeys:
            if key.frame == nextKey.frame:
                # Interpolate the values to cross the rest value (0) smoothly
                insertedValueToRest = (key.value + restValue) / 2
                nextValueFromRest = (nextKey.value + restValue) / 2
                
                # Adjust the current keyframe value to fade out to rest value
                key.value = insertedValueToRest

                # Adjust the next keyframe value to fade in from rest value
                nextKey.value = nextValueFromRest

    # Identify non-overlapping keyframes in nextKeys
    nonOverlappingNextKeys = [key for key in nextKeys if key.frame not in [k.frame for k in keysOverlapping]]

    # Combine non-overlapping nextKeys with insertedKeys
    finalKeyframes = []
    i, j = 0, 0
    while i < len(insertedKeys) and j < len(nonOverlappingNextKeys):
        if insertedKeys[i].frame < nonOverlappingNextKeys[j].frame:
            finalKeyframes.append(insertedKeys[i])
            i += 1
        else:
            finalKeyframes.append(nonOverlappingNextKeys[j])
            j += 1

    # Add any remaining keyframes
    finalKeyframes.extend(insertedKeys[i:])
    finalKeyframes.extend(nonOverlappingNextKeys[j:])

    # Update insertedKeys with final combined keyframes
    insertedKeys.clear()
    insertedKeys.extend(finalKeyframes)


def pruneKeyframes(insertedKeys: List[Keyframe], nextKeys: List[Keyframe]) -> None:
    keysOverlapping = findOverlap(insertedKeys, nextKeys)

    # Prune strategy: remove the last couple of keyframes from insertedKeys
    if keysOverlapping:
        last_overlap_frame = max(key.frame for key in keysOverlapping)
        
        # Identify keyframes in insertedKeys that overlap and are near the end
        prunedInsertedKeys = [key for key in insertedKeys if key.frame <= last_overlap_frame]
        
        # If there's overlap, we'll remove the last keyframe or two
        if len(prunedInsertedKeys) > 1:
            prunedInsertedKeys.pop()  # Remove the last overlapping keyframe
            if len(prunedInsertedKeys) > 1:
                prunedInsertedKeys.pop()  # Optionally remove one more for smoother transition

        # Remaining keyframes
        remainingInsertedKeys = [key for key in insertedKeys if key.frame > last_overlap_frame]
        
        # Final keyframes after pruning
        finalKeyframes = prunedInsertedKeys + remainingInsertedKeys + nextKeys
    else:
        # If no overlap, just concatenate the keys
        finalKeyframes = insertedKeys + nextKeys

    # Sort and update the insertedKeys with pruned results
    finalKeyframes.sort(key=lambda keyframe: keyframe.frame)
    insertedKeys.clear()
    insertedKeys.extend(finalKeyframes)


