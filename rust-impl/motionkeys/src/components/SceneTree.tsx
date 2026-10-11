import { Icon, IconName } from "../icons";

// MARK: - Items

// a row in the tree, its children are only made when it's open
type Item = { key: string; icon: IconName; label: string; value?: string; children?: () => Item[] };

const list = (value: any): any[] => (Array.isArray(value) ? value : []);
const num = (value: any): string => (typeof value === "number" ? value.toFixed(2) : "");
const vector = (v: any, f: (n: any) => string = num): string => (v ? [v.x, v.y, v.z].map(f).join(", ") : "");
const degrees = (n: any): string => (typeof n === "number" ? `${((n * 180) / Math.PI).toFixed(1)}°` : "");
// keyframe times come in seconds
const seconds = (n: any): string => (typeof n === "number" ? `${Number(n.toFixed(2))}s` : "");

// shape keys come as Blender reprs, `bpy.data.shape_keys['Key'].key_blocks["Basis"]`
const shapeKeyName = (repr: string): string => /\[["']([^"'\]]*)["']\]$/.exec(repr)?.[1] ?? repr;

// a curve named like Blender's graph editor names it: `X Location`, `Value (Pressed)`
const AXES = ["X", "Y", "Z", "W"];
const PROPERTIES: Record<string, string> = { location: "Location", rotation_euler: "Euler Rotation", rotation_quaternion: "Quaternion Rotation", scale: "Scale" };
function curveName(curve: any): string {
    const path: string = curve?.data_path ?? "";
    const index: number = curve?.array_index ?? 0;
    if (PROPERTIES[path]) return `${AXES[index] ?? index} ${PROPERTIES[path]}`;
    const key = /^key_blocks\["(.*)"\]\.value$/.exec(path);
    if (key) return `Value (${key[1]})`;
    return `${path}[${index}]`;
}

function objectItem(object: any, key: string): Item {
    return {
        key,
        icon: "object",
        label: object?.name ?? "",
        children: () => {
            const items: Item[] = [
                {
                    key: `${key}/transform`,
                    icon: "transform",
                    label: "Transform",
                    children: () => [
                        { key: `${key}/position`, icon: "position", label: "Position", value: vector(object.position) },
                        { key: `${key}/rotation`, icon: "rotation", label: "Rotation", value: vector(object.rotation, degrees) },
                        { key: `${key}/scale`, icon: "scale", label: "Scale", value: vector(object.scale) },
                    ],
                },
            ];

            // the reference key isn't one of the keys
            const shapes = object?.blend_shapes;
            const names = [shapes?.reference, ...list(shapes?.keys)].filter((name) => typeof name === "string").map(shapeKeyName);
            if (names.length) {
                items.push({ key: `${key}/shape_keys`, icon: "shape_keys", label: "Shape Keys", children: () => names.map((name, i) => ({ key: `${key}/shape_keys/${i}`, icon: "shape_key", label: name })) });
            }

            const curves = list(object?.anim_curves);
            if (curves.length) {
                items.push({
                    key: `${key}/animation`,
                    icon: "animation",
                    label: "Animation",
                    children: () =>
                        curves.map((curve, i) => ({
                            key: `${key}/animation/${i}`,
                            icon: "fcurve",
                            label: curveName(curve),
                            children: () => list(curve?.keyframe_points).map((point, j) => ({ key: `${key}/animation/${i}/${j}`, icon: "keyframe", label: seconds(point?.co?.[0]), value: num(point?.co?.[1]) })),
                        })),
                });
            }
            return items;
        },
    };
}

function sceneItem(scene: any): Item {
    return {
        key: "scene",
        icon: "scene",
        label: scene?.name ?? "",
        children: () =>
            list(scene?.object_groups).map((group, i) => ({
                key: `scene/${i}`,
                icon: "collection",
                label: group?.name ?? "",
                children: () => list(group?.objects).map((object, j) => objectItem(object, `scene/${i}/${j}`)),
            })),
    };
}

// MARK: - Tree

// the scene Scene Link gives, as a tree of its collections, objects and their data. only shown, rows open and close
function SceneTree({ scene, expanded, onToggle }: { scene: any; expanded: Set<string>; onToggle: (key: string) => void }) {
    const rows: JSX.Element[] = [];

    const add = (item: Item, depth: number) => {
        const children = expanded.has(item.key) ? item.children?.() : undefined;
        // a row that could have children gets a chevron, even one that turns out empty
        const parent = item.children !== undefined;
        rows.push(
            <div key={item.key} className="scene-tree-row" style={{ paddingLeft: depth * 14 }} onClick={() => parent && onToggle(item.key)}>
                <span className="scene-tree-chevron">
                    {parent && (
                        <svg height="6" viewBox="0 0 10 6" style={{ transform: expanded.has(item.key) ? undefined : "rotate(-90deg)" }}>
                            <path d="M1 0.5l4 4 4-4" fill="none" stroke="currentColor" />
                        </svg>
                    )}
                </span>
                <Icon name={item.icon} />
                <span className="scene-tree-label">{item.label}</span>
                {item.value && <span className="scene-tree-value">{item.value}</span>}
            </div>
        );
        children?.forEach((child) => add(child, depth + 1));
    };
    add(sceneItem(scene), 0);

    return <>{rows}</>;
}

export default SceneTree;
