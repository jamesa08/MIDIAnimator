import sys
sys.path.append(".")

from MIDIAnimator.src.MIDIFile import *
from MIDIAnimator.utils.functions import noteToName, nameToNote
import unittest

class MIDIAnimatorTest(unittest.TestCase):
    def testImportingMIDI(self):
        # TODO: fix this
        # also test type 0 and type 1, and other edge case files
        node = MIDIFile("tests/two_notes_CC_simple.mid")

        data = node.getMIDIData()
        for instrument in data:
            for track in instrument:
                self.assertEqual(track.name, "Track 1")
                self.assertEqual(track.index, 0)
                self.assertEqual(track.timeOn, [0.0, 0.5])
                self.assertEqual(track.timeOff, [2.103125, 2.603125])
                self.assertEqual(track.noteNumber, [48, 45])
                self.assertEqual(track.velocity, [98, 98])
                self.assertEqual(track.controlChange, {20: [(0, 127, 0.0), (0, 127, 4.0)]})

    def testNoteToName(self):
        self.assertEqual(noteToName(60), "C3")
        self.assertEqual(noteToName(61), "C#3")
    
    def testNameToNote(self):
        self.assertEqual(nameToNote("C3"), 60)
        self.assertEqual(nameToNote("C#3"), 61)
    
    def testImportLocalMido(self):
        from MIDIAnimator.libs import mido
        self.assertEqual(mido.__file__.split("/")[-3], "libs")

    # Tests with animation data should be handeled elsewhere (shell scripts?)


def main():
    try:
        unittest.main()
    except SystemExit as inst:
        # raised by sys.exit(True) when tests failed
        if inst.args[0] is True:
            raise


if __name__ == "__main__":
    main()