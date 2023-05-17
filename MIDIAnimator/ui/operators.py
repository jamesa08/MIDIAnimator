from ..utils import nameToNote, noteToName, convertNoteNumbers
from ..utils.blender import FCurvesFromObject
from .. utils.logger import logger, buffer
from .. import bl_info
from operator import attrgetter
from ast import literal_eval
import platform
import sys
import bpy

def col_sort_key(obj):
    """Split collection names by _ and sort by MIDINote or the name of the note"""
    name = obj.name.split("_")[-1]
    try:
        return convertNoteNumbers(name)[0]
    except ValueError:
        raise ValueError(f"Object '{obj.name}' has an invalid note number or name") from None

class SCENE_OT_quick_add_props(bpy.types.Operator):
    bl_idname = "scene.quick_add_props"
    bl_label = "Assign Notes to Objects"
    bl_description = "This will map the list of MIDINotes to a sorted collection of objects."
    
    def preExecute(self, context):
        scene = context.scene
        sceneMidi = scene.midi
        col = sceneMidi.quick_obj_col

        # convert String "list" into type list

        try:
            if len(sceneMidi.quick_note_number_list) != 0 and sceneMidi.quick_note_number_list != "[]":
                note_numbers = literal_eval(sceneMidi.quick_note_number_list)
            elif not sceneMidi.quick_sort_by_name or sceneMidi.quick_note_number_list == "[]":
                note_numbers = [col_sort_key(obj) for obj in col.all_objects]
            else:
                error_message = "List/collection contains errors. Please check the list and try again."
                self.report({"ERROR"}, error_message)
                logger.error(error_message)
                return {'CANCELLED'}
        except Exception as e:
            self.report({"ERROR"}, f"List/collection contains errors. Please check the list and try again. Error message: {e}.")
            logger.error(f"List/collection contains errors. Please check the list and try again. Error message: {e}.")
            return {'CANCELLED'}
        
        if note_numbers is not None:
            if len(note_numbers) != len(col.all_objects):
                self.report({"ERROR"}, f"Objects and note numbers unbalanced! \nObjects: {len(col.all_objects)}, Note Numbers: {len(note_numbers)}")
                logger.error(f"Objects and note numbers unbalanced! \nObjects: {len(col.all_objects)}, Note Numbers: {len(note_numbers)}")
                return {"CANCELLED"}

            sortedObjs = sorted(col.all_objects, key=attrgetter('name')) if sceneMidi.quick_sort_by_name else sorted(col.all_objects, key=col_sort_key)
            
            return (note_numbers, sortedObjs)
    
    def mapNoteToObjStr(self, note_numbers, objs):
        l = []
        for noteNum, obj in zip(note_numbers, objs):
            if isinstance(noteNum, str): noteNum = convertNoteNumbers(str(noteNum))[0]
            l.append(f"Object: {obj.name} => Note: {noteNum}/{noteToName(noteNum)}")
        
        return "\n".join(l)

    def execute(self, context):
        scene = context.scene
        sceneMidi = scene.midi
        if "CANCELLED" in self.preExecute(context): 
            return {"CANCELLED"}  # make sure this actually cancells
        note_numbers, sortedObjs = self.preExecute(context)

        sceneMidi.quick_note_number_list = str(note_numbers)

        for noteNumber, obj in zip(note_numbers, sortedObjs):
            obj.midi['note_number'] = str(noteNumber)
        
        return {'FINISHED'}

    def invoke(self, context, event):
        if "CANCELLED" in self.preExecute(context): 
            return {"CANCELLED"}  # make sure this actually cancells
        
        return context.window_manager.invoke_props_dialog(self)

    def draw(self, context):
        row = self.layout
        note_numbers, sortedObjs = self.preExecute(context)
        table = self.mapNoteToObjStr(note_numbers, sortedObjs)

        for line in table.split("\n"):
            row.label(text=line)

class SCENE_OT_copy_log(bpy.types.Operator):
    bl_idname = "scene.copy_log"
    bl_label = "Copy Output"
    bl_description = "This will grab the output from the console and copy it to the clipboard."

    def execute(self, context):
        output = buffer.getvalue() + f"""\n#-#-#-#-#-# SYSTEM SPECS #-#-#-#-#-# 
MIDIAnimator version {'.'.join([str(i) for i in bl_info['version']])} {bl_info['name'].replace('MIDIAnimator ', '')}.
Blender version {bpy.app.version_string}.
Python version {sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}.
OS: {platform.platform()}.
System: {platform.system()}.
Version: {platform.version()}."""

        context.window_manager.clipboard = output
        logger.info("Copied output to clipboard.")
        self.report({'INFO'}, "Copied MIDIAnimator output to clipboard.")
        return {'FINISHED'}
