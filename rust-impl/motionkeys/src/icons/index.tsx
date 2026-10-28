// the interface's icons by what they stand for. Blender 2.79 and Silk icons, fetched by scripts/fetch_icons.py (see CREDITS)
import { useId, useMemo } from "react";
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
import chartCurve from "./silk/chart_curve.svg?raw";
import layoutSidebar from "./silk/layout_sidebar.svg?raw";
import disk from "./silk/disk.svg?raw";
import folderPage from "./silk/folder_page.svg?raw";
import add from "./silk/add.svg?raw";
import arrowRedoMirrored from "./silk/arrow_redo_mirrored.svg?raw";
import arrowRedo from "./silk/arrow_redo.svg?raw";
import shapeGroup from "./silk/shape_group.svg?raw";
import shapeUngroup from "./silk/shape_ungroup.svg?raw";
import shapeGroupGo from "./silk/shape_group_go.svg?raw";
import shapeGroupCome from "./silk/shape_group_come.svg?raw";
import shapeGroupHouse from "./silk/shape_group_house.svg?raw";
import shapeGroupMagnifier from "./silk/shape_group_magnifier.svg?raw";
import shapeHandlesMagnifier from "./silk/shape_handles_magnifier.svg?raw";
import controlPlayGreen from "./silk/control_play_green.svg?raw";

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
    hide_panels: layoutSidebar,
    save: disk,
    open: folderPage,
    add: add,
    undo: arrowRedoMirrored,
    redo: arrowRedo,
    group: shapeGroup,
    ungroup: shapeUngroup,
    edit_group: shapeGroupGo,
    exit_group: shapeGroupCome,
    exit_to_root: shapeGroupHouse,
    frame_all: shapeGroupMagnifier,
    frame_selected: shapeHandlesMagnifier,
    run: controlPlayGreen,
};

export type IconName = keyof typeof ICONS;

// a 16px icon, `className` sizes or turns it. Silk's svgs are drawn inline: webkit draws an svg with filters (Silk's inner
// shadows) through an img at 1x, blurred on a retina screen
export function Icon({ name, className }: { name: IconName; className?: string }) {
    const source: string = ICONS[name];
    const classes = className ? `icon ${className}` : "icon";
    if (source.startsWith("<svg")) return <InlineSvg svg={source} className={classes} />;
    return <img className={classes} src={source} alt="" draggable={false} />;
}

// each copy gets ids of its own, the ones two icons share would otherwise all point at the first
function InlineSvg({ svg, className }: { svg: string; className: string }) {
    const prefix = useId().replace(/[^\w-]/g, "");
    const html = useMemo(
        () =>
            svg
                .replace(/id="([^"]+)"/g, `id="${prefix}$1"`)
                .replace(/url\(#([^)]+)\)/g, `url(#${prefix}$1)`)
                .replace(/href="#([^"]+)"/g, `href="#${prefix}$1"`),
        [svg, prefix]
    );
    return <span className={className} dangerouslySetInnerHTML={{ __html: html }} />;
}
