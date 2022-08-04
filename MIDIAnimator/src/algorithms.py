from __future__ import annotations
import bpy
from typing import List, Tuple, TYPE_CHECKING
from math import sin, cos, pi, e
from .. data_structures.midi import MIDITrack

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

def genDampedOscKeyframes(period: float, amplitude: float, damp: float, frameRate: float) -> List[Tuple[float, float]]:
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
    
    out = [(0, 0)]
    
    while abs(y) > 0.01:
        approxX = (i + 1/2) * (pi / period) * frameRate
        i += 1
        
        x = newtonsMethod(newtonsMethod(newtonsMethod(newtonsMethod(approxX))))
        if x < 0: continue
        y = waveFunc(x)
        
        out.append((x, waveFunc(x)))
    
    out.append((x + 1, 0))
    
    return out
