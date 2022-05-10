__all__ = [
    "noteToName",
    "nameToNote",
    "gmProgramToName",
    "gmPercProgramToName"
]

from . gmInstrumentMap import _gmInst

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

def gmProgramToName(pcNum: int) -> str:
    """Takes a General MIDI program change number from 1-128 and returns the GM instrument name.

    :param int pcNum: program change number
    :return str: GM instrument name
    """
    assert isinstance(pcNum, int), "please pass in a int"
    assert 1 <= int(pcNum) <= 128, "program change number out of range!" # TODO: is this range correct? check mido
    return _gmInst[int(pcNum)]