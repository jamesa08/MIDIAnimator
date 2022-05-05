def noteToName(nVal: int) -> str:
    assert isinstance(nVal, int), "please pass in an int"
    names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
    return f"{names[nVal % 12]}{(nVal // 12) - 2}"


def nameToNote(nStr: str) -> int:
    assert isinstance(nStr, str), "please pass in a string"
    names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
    return (names.index(nStr[0] + (nStr[1] if nStr[1] == '#' else ''))) + ((int(nStr[2:]) if nStr[1] == '#' else int(nStr[1:])) + 2) * 12