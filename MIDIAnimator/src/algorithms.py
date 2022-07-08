from __future__ import annotations
import bpy
from typing import List, TYPE_CHECKING
from math import cos, pi
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

def findMaxSticks(track: MIDITrack, sticks: List) -> int:
    """
    :param intervals: List[FrameRange]
    :return int: max number of sticks needed
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

    for stick in sticks:
        for note in track.notes:
            if note.noteNumber not in stick.settings.listenNotes: continue
            print(stick.settings.__name__, note)
            
        

    return None

def animateSine(time: float, startVal: float, endVal: float, duration: float) -> float:
    # https://www.desmos.com/calculator/kw2grve25z
    time = min(max(time, 0), duration)  # clamps time 
    return -((cos((1 / duration) * pi * time) - 1) * ((endVal - startVal) / 2)) + startVal


class Drumstick1Settings:
    listenNotes = [41, 43, 45, 47, 48, 49, 57, 59]
    
    onSpeed = 60   # ms
    onValue = -1
    
    offSpeed = 400  # ms
    offValue = 0
    
    # define values
    _valueDiff = onValue - offValue
    onslope = _valueDiff / onSpeed
    offslope = _valueDiff / offSpeed
    velocity = 0
    

class Drumstick:
    """holds data for the drumstick, and its settings"""
    onSpeed: float
    onValue: float

    offSpeed: float
    offValue: float
