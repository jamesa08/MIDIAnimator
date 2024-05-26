from typing import List, Tuple
from dataclasses import dataclass

@dataclass
class Keyframe:
    frame: float
    value: float

def findOverlap(keyList1, keyList2):
    if len(keyList2) == 0:
        return []
    
    if keyList1[0].frame > keyList2[0].frame:
        #this means a note is somehow going back in time? is this even possible?
        # notes should always be sequential, and not in reverse time
        raise ValueError("first keyframe in keyList1 is bigger than first keyframe in keyList2! Please open a issue on GitHub along with the MIDI file.")
    
    overlappingKeyList = []
    overlapping = False
    for key1 in reversed(keyList1):
        # print ("testing case key1={keyl.frame} and key2={keyList2[0].frame}")
        if key1.frame > keyList2[0].frame:
            overlapping = True
            overlappingKeyList.append(key1)
        else:
            # not overlapping
            if overlapping:
                overlappingKeyList.append(key1)
            break

    return list(reversed(overlappingKeyList))

def getValue(key1: Keyframe, key2: Keyframe, frame: float) -> float:
    x1, y1 = key1.frame, key1.value
    x2, y2 = key2.frame, key2.value    
    try:
        m = (y2 - y1) / (x2 - x1)
    except ZeroDivisionError:
        # i dont know if this will work every time
        m = 0
    
    c = y1 - m * x1
    return (m * frame) + c

def interval(keyList, frame):
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


x = 0
keyList1 = [Keyframe(frame=59, value=0.0), Keyframe(frame=74, value=0.5), Keyframe(frame=77, value=0.5), Keyframe(frame=92, value=0.0)]
keyList2 = [Keyframe(frame=89-x, value=0.0), Keyframe(frame=104-x, value=0.5), Keyframe(frame=107-x, value=0.5), Keyframe(frame=122-x, value=0.0)]

keyList1Overlapping = findOverlap(keyList1, keyList2)

key1InterpolatedValues = []
key2InterpolatedValues = []

# interpolate the keyframes for each graph
for key2 in keyList2:
    key1interval1, key1interval2 = interval(keyList1Overlapping, key2.frame)
    if key1interval1 is None and key1interval2 is None: 
        continue
    key2InterpolatedValues.append(Keyframe(key2.frame, getValue(key1interval1, key1interval2, key2.frame)))

for key1 in keyList1Overlapping:
    key2interval1, key2interval2 = interval(keyList2, key1.frame)
    if key2interval1 is None and key2interval2 is None: 
        continue
    key1InterpolatedValues.append(Keyframe(key1.frame, getValue(key2interval1, key2interval2, key1.frame)))

# now add the keyframe values together (the most important part)
for key1, key1Interp in zip(keyList1Overlapping, key1InterpolatedValues):
    key1.value += key1Interp.value

for key2, key2Interp in zip(keyList2, key2InterpolatedValues):
    key2.value += key2Interp.value

# extend the lists (need a better method to ensure the keyframes before get cut off and then start )
keyList1Overlapping.extend(keyList2)

keyList1Overlapping.sort(key=lambda x: x.frame)

print("x, y")
for key in keyList1Overlapping:
    print(f"{key.frame}, {key.value}")


    
