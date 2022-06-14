from ast import literal_eval
from ..utils.functions import nameToNote, noteToName
from ..utils.blender import FCurvesFromObject
from operator import attrgetter
import bpy

def col_sort_key(obj):
    """Split collection names by _ and sort by MIDINote or the name of the note"""
    name = obj.name.split("_")[-1]

    try:
        return int(name)
    except ValueError:
        try: return nameToNote(name)
        except Exception: raise RuntimeError(f"Name {obj.name} is not valid for object {obj}. Names should follow the format `name_C3` or `name_60`")
    except Exception as e:
        print(f"WARNING: {e}")
        print("Please open an issue on the GitHub.")
        raise e

class SCENE_OT_quick_add_props(bpy.types.Operator):
    bl_idname = "scene.quick_add_props"
    bl_label = "Assign Notes to Objects"
    bl_description = "This will map the list of MIDINotes to a sorted collection of objects."

    def mapNoteToObjStr(self, note_numbers, objs):
        l = []
        for noteNum, obj in zip(note_numbers, objs):
            l.append(f"Object: {obj.name} => Note: {noteNum}/{noteToName(noteNum)}")
        
        return "\n".join(l)

    def preExecute(self, context):
        scene = context.scene
        col = scene.quick_obj_col

        # make sure essential vars are filled
        variables = (scene.quick_obj_col, scene.quick_obj_curve, scene.quick_obj_curve_index)

        for v in variables:
            if v is None: 
                self.report({"ERROR"}, f"One of the properties has no data! Please fill any missing data and try again.")
                return {"CANCELLED"}

        # make sure Animation Curve has FCurves
        try: FCurvesFromObject(scene.quick_obj_curve)[context.scene.quick_obj_curve_index]
        except IndexError: self.report({"ERROR"}, "FCurve does not have specified animation index!")
        except AttributeError: self.report({"ERROR"}, "Object has no animation!")

        # convert String "list" into type list

        try:
            note_numbers = literal_eval(context.scene.note_number_list)
            # context.scene['note_number_list'] = str(sorted(note_numbers))
        except Exception as e:
            note_numbers = None
            self.report({"ERROR"}, f"List contains errors. Please check the list and try again. Error message: \"{e}\".")
            return {'CANCELLED'}
        
        if note_numbers is not None:
            assert len(note_numbers) == len(col.all_objects), "Objects and list unbalanced!"
            
            sortedObjs = sorted(col.all_objects, key=attrgetter('name')) if scene.quick_use_sorted else sorted(col.all_objects, key=col_sort_key)
            
            return (sorted(note_numbers), sortedObjs)
            

    def execute(self, context):
        scene = context.scene
        note_numbers, sortedObjs = self.preExecute(context)

        col = scene.quick_obj_col
        col.instrument_type = scene.quick_instrument_type

        context.scene['note_number_list'] = str(sorted(note_numbers))

        for noteNumber, obj in zip(sorted(note_numbers), sortedObjs):
            obj['note_number'] = str(noteNumber)
            obj['animation_curve'] = scene.quick_obj_curve
            obj['animation_curve_index'] = scene.quick_obj_curve_index
            obj['note_hit_time'] = scene.quick_note_hit_time
        
        return {'FINISHED'}

    def invoke(self, context, event):
        return context.window_manager.invoke_props_dialog(self)

    def draw(self, context):
        row = self.layout
        note_numbers, sortedObjs = self.preExecute(context)
        table = self.mapNoteToObjStr(sorted(note_numbers), sortedObjs)

        for line in table.split("\n"):
            row.label(text=line)