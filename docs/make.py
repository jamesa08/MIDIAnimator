# because you cannot change how autodoc works, I wrote a custom stripper for the API docs that gets called before the docs are built.
# This is a bit of a hack, but it works.

import os

for filename in os.listdir('source/api/'):
    if ".libs." in filename\
    or "MIDIAnimator.rst" == filename\
    or "modules.rst" == filename:
        os.remove(os.path.join('source/api/', filename))
    
# now open the files up and edit them
# the first line of the file will be the title of the module. We want to replace " package" with nothing, and "MIDIAnimator." with nothing. Still keep the old name (With the " package" replacement), as we want to check this against other lines
# if the first two characters are "..", don't do anything (as this is an RST module), write the line and continue
# if the line contains "Submodules" and the next line after that contains a dash, do not write either of those lines
# if any line contains the first line of the file as described above + ".", replace it with nothing

for filename in os.listdir('source/api/'):
    if ".libs." in filename\
    or "MIDIAnimator.rst" == filename\
    or "modules.rst" == filename:
        continue
    with open(os.path.join('source/api/', filename), 'r') as f:
        lines = f.readlines()

    contains = False
    title = ""
    with open(os.path.join('source/api/', filename), 'w') as f:
        for i, line in enumerate(lines):
            if i == 0:
                title = line.replace(" package", "")
                f.write(title.replace("MIDIAnimator.", ""))
                title = title.strip()
                continue
            if line.startswith(".."):
                f.write(line)
                continue
            if "Submodules" in line:
                if lines[lines.index(line) + 1].startswith("-"):
                    contains = True
                    continue

            if contains:
                contains = False
                continue

            if title in line:
                line = line.replace(title + ".", "")
            if " module" in line:
                # replace with ".py"
                line = line.replace(" module", ".py")
            f.write(line)

print("Sucessfully stripped signatures from API docs.")