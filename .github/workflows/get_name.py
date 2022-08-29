import sys
import argparse

parser = argparse.ArgumentParser(description="pull bl_info out of init file and concatenate a name")
parser.add_argument("--readFile")

args = parser.parse_args(sys.argv[1:])

# this is so hacky just to pull bl_info out, but it works!
with open(args.readFile) as file:
    lines = file.readlines()
    
    state = False
    bl_info_lines = []
    
    for line in lines:
        line = line.strip()
        if "bl_info" in line:
            state = True

        if state:
            bl_info_lines.append(line)
        
        if line == "}":
            break

bl_info = {}
exec("\n".join(bl_info_lines))

names_split = bl_info['name'].split(" ")
names_split.insert(2, "-".join([str(val) for val in bl_info['version']]))
out = "-".join(names_split)

print(out)