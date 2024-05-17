JSON_DATA = r""""""
# JSON_DATA is a static variable that is injected when rust function send_scene_data() is called
# also has to be a raw triple quote string

import bpy
import json

def execute():
    print("json", JSON_DATA)
    data = json.loads(JSON_DATA)
    
    for scene_name, scene_value in data.items():
        print(f"Scene: {scene_name}")

        for object_group in scene_value['object_groups']:
            print(f"  Object Group: {object_group['name']}")

            # Iterate over objects within each object group
            for obj in object_group['objects']:
                print(f"    Object: {obj['name']}")
                print(f"      Position: {obj['position']}")
                print(f"      Rotation: {obj['rotation']}")
                print(f"      Scale: {obj['scale']}")

                # If there are blend shapes
                blend_shapes = obj.get('blend_shapes', {})
                if blend_shapes:
                    print(f"      Blend Shapes: {blend_shapes}")

                # If there are animation curves
                anim_curves = obj.get('anim_curves', [])
                if anim_curves:
                    for curve in anim_curves:
                        print(f"      Animation Curve:")
                        print(f"        Array Index: {curve.get('array_index')}")
                        print(f"        Auto Smoothing: {curve.get('auto_smoothing')}")
                        print(f"        Data Path: {curve.get('data_path')}")
                        print(f"        Extrapolation: {curve.get('extrapolation')}")
                        print(f"        Range: {curve.get('range')}")
                        
                        # Iterate over keyframe points
                        keyframe_points = curve.get('keyframe_points', [])
                        for point in keyframe_points:
                            print(f"        Keyframe Point:")
                            print(f"          Amplitude: {point.get('amplitude')}")
                            print(f"          Back: {point.get('back')}")
                            print(f"          Easing: {point.get('easing')}")
                            print(f"          Handle Left: {point.get('handle_left')}")
                            print(f"          Handle Left Type: {point.get('handle_left_type')}")
                            print(f"          Handle Right: {point.get('handle_right')}")
                            print(f"          Handle Right Type: {point.get('handle_right_type')}")
                            print(f"          Interpolation: {point.get('interpolation')}")
                            print(f"          Coordinate: {point.get('co')}")
                            print(f"          Period: {point.get('period')}")
                        
    return "OK"