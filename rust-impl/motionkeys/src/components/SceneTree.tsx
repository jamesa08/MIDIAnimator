import { Icon, IconName } from "../icons";
import { channelLabel, channelTitle } from "../utils/objectGroups";

// MARK: - Items

// a row in the tree, its children are only made when it's open
type Item = { key: string; icon: IconName; label: string; value?: string; children?: () => Item[] };

const list = (value: any): any[] => (Array.isArray(value) ? value : []);
const num = (value: any): string => (typeof value === "number" ? value.toFixed(2) : "");
const vector = (v: any, f: (n: any) => string = num): string => list(v).map(f).join(", ");
const degrees = (n: any): string => (typeof n === "number" ? `${((n * 180) / Math.PI).toFixed(1)}°` : "");
// keyframe times come in seconds
const seconds = (n: any): string => (typeof n === "number" ? `${Number(n.toFixed(2))}s` : "");

// `inDegrees`: the scene's rotations are in degrees already, not radians
function objectItem(object: any, key: string, inDegrees: boolean): Item {
    return {
        key,
        icon: "object",
        label: object?.name ?? "",
        children: () => {
            const items: Item[] = [];
            const transform = object?.transform;
            if (transform) {
                items.push({
                    key: `${key}/transform`,
                    icon: "transform",
                    label: "Transform",
                    children: () => [
                        { key: `${key}/position`, icon: "position", label: "Position", value: vector(transform.position) },
                        { key: `${key}/rotation`, icon: "rotation", label: "Rotation", value: vector(transform.rotation, inDegrees ? (n) => (typeof n === "number" ? `${n.toFixed(1)}°` : "") : degrees) },
                        { key: `${key}/scale`, icon: "scale", label: "Scale", value: vector(transform.scale) },
                    ],
                });
            }

            // the channels the bridge lists, by their groups
            const channels = list(object?.channels).filter((channel) => typeof channel?.id === "string");
            if (channels.length) {
                const groups = [...new Set(channels.map((channel) => `${channel.group ?? ""}`))];
                items.push({
                    key: `${key}/channels`,
                    icon: "fcurve",
                    label: "Channels",
                    children: () =>
                        groups.map((group) => ({
                            key: `${key}/channels/${group}`,
                            icon: "group",
                            label: group,
                            children: () => channels.filter((channel) => `${channel.group ?? ""}` === group).map((channel) => ({ key: `${key}/channels/${group}/${channel.id}`, icon: "fcurve", label: channelTitle(channel) })),
                        })),
                });
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
                            label: channelLabel(object, typeof curve?.channel === "string" ? curve.channel : ""),
                            children: () => list(curve?.keys).map((point, j) => ({ key: `${key}/animation/${i}/${j}`, icon: "keyframe", label: seconds(point?.[0]), value: num(point?.[1]) })),
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
                children: () => list(group?.objects).map((object, j) => objectItem(object, `scene/${i}/${j}`, scene?.axes?.angle_unit === "degrees")),
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
