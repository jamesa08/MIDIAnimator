import bpy
from typing import List, Tuple

def maxNeeded(intervals: List[Tuple[int, int]]) -> int:
    # keep track of maximum number of active items
    maxCount = 0

    # number of active items currently
    activeCount = 0
    # list of end times for currently active items sorted by the end time
    endTimesForActive = []

    # for testing code
    # currentActives = []

    # for each (start frame, end frame) interval for objects
    for start, end, cls in intervals:

        endIndex  = 0
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


# def animateProjectile(frame, frameOn, frameOff, cls, cache: ProjectileCache) -> None:
#     # fill in x from object
#     # get an object that is not being used (somehow)
#     objName = cls._getReusableProjectile()
#     objName = f"{hex(id(cls))}_{cls._note}"  # TEMP

#     hitTime = cls._blenderObject.note_hit_time
#     delta = frame - frameOn

#     x = cls._blenderObject.location[0]
#     y, z = cls.positionForFrame(delta)

#     bpy.data.objects[objName].location = (x, y, z)
#     bpy.data.objects[objName].keyframe_insert(data_path="location", frame=frame)

def animateString(frame, frameOn, frameOff, cls) -> None:
    pass