// the interface's icons by what they stand for. Blender 2.79 and Silk icons, fetched by scripts/fetch_icons.py (see CREDITS)
import sceneData from "./blender/scene_data.png";
import group from "./blender/group.png";
import objectData from "./blender/object_data.png";
import manipul from "./blender/manipul.png";
import manTrans from "./blender/man_trans.png";
import manRot from "./blender/man_rot.png";
import manScale from "./blender/man_scale.png";
import shapekeyData from "./blender/shapekey_data.png";
import keyHlt from "./blender/key_hlt.png";
import action from "./blender/action.png";
import space2 from "./blender/space2.png";
import chartCurve from "./silk/chart_curve.svg";

const ICONS = {
    scene: sceneData,
    collection: group,
    object: objectData,
    transform: manipul,
    position: manTrans,
    rotation: manRot,
    scale: manScale,
    shape_keys: shapekeyData,
    shape_key: keyHlt,
    animation: action,
    fcurve: chartCurve,
    keyframe: space2,
};

export type IconName = keyof typeof ICONS;

// a 16px icon
export function Icon({ name }: { name: IconName }) {
    return <img className="icon" src={ICONS[name]} alt="" draggable={false} />;
}
