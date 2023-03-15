from . gmInstrumentMap import _gmInst
from math import sin, cos, pi, e, sqrt, asin, log
from typing import Tuple, List
from re import search as reSearch


def noteToName(nVal: int) -> str:
    """Takes a MIDI note number and returns the name
    Example: `noteToName(60)` returns `"C3"`

    :param int nVal: the MIDI note number
    :return str: the note name
    """
    assert isinstance(nVal, int), "please pass in an int"
    
    names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
    return f"{names[nVal % 12]}{(nVal // 12) - 2}"


def nameToNote(nStr: str) -> int:
    """Takes a name and returns the MIDI note number
    Example: `nameToNote("C3")` returns `60`

    :param str nStr: the note name
    :return int: the MIDI note number
    """
    assert isinstance(nStr, str), "please pass in a string"
    
    names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
    return (names.index(nStr[0] + (nStr[1] if nStr[1] == '#' else ''))) + ((int(nStr[2:]) if nStr[1] == '#' else int(nStr[1:])) + 2) * 12


def convertNoteNumbers(inputStr: str):
    """converts a note number string "c4" or "60" to a MIDI Note number (integer)
    Will work with more than 1 input if the values are seperated via commas

    :param str inputStr: the input note number string, e.g., "c3" or "60"
    :raises ValueError: if the string is invalid
    :return int or Tuple[int]: a MIDI Note number (int)
    """
    if reSearch("^[0-9]+$", inputStr):
        return (int(inputStr),)
    elif reSearch("^[A-Ga-g]-?#?-?[0-8]+$", inputStr):
        return (nameToNote(inputStr),)
    elif "," in inputStr:
        return tuple([convertNoteNumbers(num.strip())[0] for num in inputStr.split(",") if num])
    else:
        raise ValueError(f"'{inputStr}' has an invalid note number or name.")

def typeOfNoteNumber(inputStr: str):
    """Figures out if a string is a MIDI note number or a name of a note
    Will work with more than 1 input if the values are seperated via commas

    :param str inputStr: the input note number string, e.g., "c3" or "60", or "c4, 60"
    :raises ValueError: if the string is invalid
    :return str or Tuple[str]: returns "note" or "name" for the corresponding values, or a tuple if there are multiple values seperated by commas 
    """
    if reSearch("^[0-9]+$", inputStr):
        return ("note",)
    elif reSearch("^[A-Ga-g]-?#?-?[0-8]+$", inputStr):
        return ("name",)
    elif "," in inputStr:
        return tuple([convertNoteNumbers(num.strip())[0] for num in inputStr.split(",") if num])
    else:
        raise ValueError(f"'{inputStr}' has an invalid note number or name.")
        

def gmProgramToName(pcNum: int) -> str:
    """Takes a General MIDI program change number from 0-127 and returns the GM instrument name.

    :param int pcNum: program change number
    :return str: GM instrument name
    """
    assert isinstance(pcNum, int), "please pass in a int"
    assert 0 <= int(pcNum) <= 127, "program change number out of range!" 
    # TODO: is this range correct? check mido
    
    return _gmInst[int(pcNum+1)]


def _closestTempo(vals: list, n: float, sortList=False) -> tuple:
    """takes a list of tempos and times and returns the one closest to n

    :param list vals: list of values to compare to, List[(time, tempo)]
    :param float n: the time to compare to
    :param bool sortList: have the tempo list sorted, defaults to False
    :return tuple: returns the closest time value's tempo & time as a tuple, e.g. (2.50 sec, 500000 ticks)
    """
    if sortList:
        vals.sort()
    
    if len(vals) == 1:
        return vals[0]
    
    for first, second in zip(vals, vals[1:] + [(0, float('inf'))]):
        if first[0] <= n < second[0]:
            return first
    
    return second

def removeDuplicates(vals: list) -> list:
    """Removes duplicate items from a list. Useful for getting all used note numbers in a MIDI File.

    :param list vals: input list
    :return list: duplicates removed
    """
    return sorted([n for i, n in enumerate(vals) if n not in vals[:i]])

def rotateAroundCircle(radius, angle) -> Tuple[int]:
    """Takes a radius (x) and an angle (y) and will return its X and Y.

    :param float radius: radius of the circle
    :param float angle: angle of the circle
    :return Tuple[int]: X and Y of the point
    """
    x = cos(angle) * radius
    y = sin(angle) * radius
    
    return x, y

# All mapRange() functions equations originate from here:
# https://www.desmos.com/calculator/pw9tgtcq16
# thank you to rgm on Stack Overflow for the graph
# https://stackoverflow.com/questions/5731863/mapping-a-numeric-range-onto-another#comment119982185_5732390

def mapRangeLinear(value: float, inMin: float, inMax: float, outMin: float, outMax: float) -> float:
    """maps a range linearly. Useful when mapping one range of values to another.
    
    e.g., if I want to map the value 20 with the in range (0, 100) to the out range (0, 200), the return value will be 40
    taken from here: https://www.desmos.com/calculator/pw9tgtcq16

    :param float value: value to evaluate from
    :param float inMin: in minimum
    :param float inMax: in maximum 
    :param float outMin: out minimum 
    :param float outMax: out maximum
    :return float: the resulting value mapped linearlly
    """
    try:
        return outMin + (((value - inMin) / (inMax - inMin)) * (outMax - outMin))
    except ZeroDivisionError:
        return 1

def mapRangeSin(value, inMin, inMax, outMin, outMax):
    """maps a range sinusoidally. Useful when mapping one range of values to another.
    
    taken from here: https://www.desmos.com/calculator/pw9tgtcq16

    :param float value: value to evaluate from
    :param float inMin: in minimum
    :param float inMax: in maximum 
    :param float outMin: out minimum 
    :param float outMax: out maximum
    :return float: the resulting value mapped linearlly
    """
    return (-((outMax - outMin) / 2)) * cos((pi * (inMin - value)) / (inMin - inMax)) + ((outMax + outMin) / 2)
    
def mapRangeArcSin(value, inMin, inMax, outMin, outMax):
    """maps a range using an inverse sinusoidal curve. Useful when mapping one range of values to another.
    
    taken from here: https://www.desmos.com/calculator/pw9tgtcq16

    :param float value: value to evaluate from
    :param float inMin: in minimum
    :param float inMax: in maximum 
    :param float outMin: out minimum 
    :param float outMax: out maximum
    :return float: the resulting value mapped linearlly
    """
    return ((outMax - outMin) / pi) * asin((2 / (inMax - inMin)) * (value - ((inMin + inMax) / 2))) + ((outMax + outMin) / 2)

def mapRangeExp(value, inMin, inMax, outMin, outMax):
    """maps a range exponentially. Useful when mapping one range of values to another.
    
    taken from here: https://www.desmos.com/calculator/pw9tgtcq16

    :param float value: value to evaluate from
    :param float inMin: in minimum
    :param float inMax: in maximum 
    :param float outMin: out minimum 
    :param float outMax: out maximum
    :return float: the resulting value mapped linearlly
    """
    if outMin <= outMax: s = 1
    else: s = -1
    return (-s * (abs(outMin - outMax - s)) ** ((value - inMax) / (inMin - inMax))) + outMax + s

def mapRangeLog(value, inMin, inMax, outMin, outMax):
    """maps a range logarithmically. Useful when mapping one range of values to another.
    
    taken from here: https://www.desmos.com/calculator/pw9tgtcq16

    :param float value: value to evaluate from
    :param float inMin: in minimum
    :param float inMax: in maximum 
    :param float outMin: out minimum 
    :param float outMax: out maximum
    :return float: the resulting value mapped linearlly
    """
    if inMin <= inMax: p = 1
    else: p = -1
    return ((outMax - outMin) * log(abs(value - inMin + p))) / log(abs(inMax - inMin + p)) + outMin

def mapRangePara(value, inMin, inMax, outMin, outMax):
    """maps a range parabolically. Useful when mapping one range of values to another.
    
    taken from here: https://www.desmos.com/calculator/pw9tgtcq16

    :param float value: value to evaluate from
    :param float inMin: in minimum
    :param float inMax: in maximum 
    :param float outMin: out minimum 
    :param float outMax: out maximum
    :return float: the resulting value mapped linearlly
    """
    return (((outMin - outMax) * ((value - inMax) ** 2)) / (inMax - inMin) ** 2) + outMax 

def mapRangeRoot(value, inMin, inMax, outMin, outMax):
    """maps a range with a square root. Useful when mapping one range of values to another.
    
    taken from here: https://www.desmos.com/calculator/pw9tgtcq16

    :param float value: value to evaluate from
    :param float inMin: in minimum
    :param float inMax: in maximum 
    :param float outMin: out minimum 
    :param float outMax: out maximum
    :return float: the resulting value mapped linearlly
    """
    return (((outMax - outMin) / sqrt(inMax - inMin)) * sqrt(value - inMin)) + outMin

