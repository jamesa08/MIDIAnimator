# MIDIAnimator documentation

Read the docs here: https://midianimatordocs.readthedocs.io/

To build:

1. Clone the repository `git clone https://github.com/imacj/MIDIAnimator.git` and `cd MIDIAnimator`.
2. Run `pip install -r docs/requirements.txt`.
3. Run `cd docs`.
4. Run `make clean && make html` in the repository directory.
5. Open `index.html` or run `open build/html/index.html` (for Mac users).

Note: Instead of reStructuredText markdown files, this project uses MyST markdown files. For some basic information on MyST markdown, visit https://myst-parser.readthedocs.io/en/v0.15.1/sphinx/intro.html and https://jupyterbook.org/en/stable/reference/cheatsheet.html.

**Please open a PR if you want to make changes to the docs.**

<!--

Useful commands:

for building (in docs dir)
make clean && make html

for opening built html
open build/html/index.html

-->
