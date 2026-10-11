// the graph window's 3d preview: a shape (a cube, a cymbal) moved by the shown curves at the playhead, drawn with
// three.js. Blender's axes (z up), so curves' values go on as they are
import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { OBJLoader } from "three/addons/loaders/OBJLoader.js";
import { AXIS_COLORS, ShownChannel, channelValue } from "../../utils/curves";
import cymbalObj from "./shapes/cymbal.obj?raw";

// the shapes the preview can show (the graph.preview_shape setting), the first is the default. an obj is Blender's obj
// export (y up) with the world origin where the shape turns
export const SHAPES: { id: string; name: string; obj?: string }[] = [
    { id: "cube", name: "Cube" },
    { id: "cymbal", name: "Cymbal", obj: cymbalObj },
];
// edges sharper than this many degrees are drawn, so a smooth shape's faces don't all get outlines
const EDGE_ANGLE = 30;

// a shape's meshes in Blender's axes. the cube (and any shape that isn't there) is Blender's default cube, 2 across
function shapeGeometries(id: string): THREE.BufferGeometry[] {
    const obj = SHAPES.find((shape) => shape.id === id)?.obj;
    if (!obj) return [new THREE.BoxGeometry(2, 2, 2)];
    const geometries: THREE.BufferGeometry[] = [];
    new OBJLoader().parse(obj).traverse((child) => {
        // y up to z up
        if (child instanceof THREE.Mesh) geometries.push(child.geometry.rotateX(Math.PI / 2));
    });
    return geometries;
}

// the properties a shape can show and how many values each has
const PROPERTIES = { location: 3, rotation_euler: 3, rotation_quaternion: 4, scale: 3 };
// the grid's size and how many lines across it
const GRID_SIZE = 20;
const GRID_DIVISIONS = 20;

// where the shape is, Blender's object properties. `quaternion` when a curve turns it by quaternion, like an object in
// quaternion rotation mode
type Transform = { location: number[]; rotation_euler: number[]; rotation_quaternion: number[]; scale: number[]; quaternion: boolean };

// the shape's transform at `time` in seconds: the first shown channel for each value, the rest left at Blender's defaults
function transformAt(channels: ShownChannel[], time: number): Transform {
    const transform: Transform = { location: [0, 0, 0], rotation_euler: [0, 0, 0], rotation_quaternion: [1, 0, 0, 0], scale: [1, 1, 1], quaternion: false };
    const set = new Set<string>();
    for (const channel of channels) {
        const property = channel.data_path as keyof typeof PROPERTIES;
        const id = `${property}[${channel.array_index}]`;
        if (!(property in PROPERTIES) || channel.array_index >= PROPERTIES[property] || set.has(id)) continue;
        const value = channelValue(channel, time);
        if (value === null) continue;
        transform[property][channel.array_index] = value;
        transform.quaternion ||= property === "rotation_quaternion";
        set.add(id);
    }
    return transform;
}

type ScenePreviewProps = { channels: ShownChannel[]; time: number; shape: string; showGrid: boolean };

// drag orbits around the middle, shift drag pans, the wheel zooms. drawn when something changes, not every frame
function ScenePreview({ channels, time, shape, showGrid }: ScenePreviewProps) {
    const containerRef = useRef<HTMLDivElement>(null);
    // the scene's parts, made once with the view
    const view = useRef<{ render: () => void; object: THREE.Object3D; floor: THREE.Object3D; surface: THREE.Material; edge: THREE.Material } | null>(null);

    // MARK: Scene

    useEffect(() => {
        const container = containerRef.current;
        if (!container) return;
        const renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setPixelRatio(window.devicePixelRatio || 1);
        renderer.domElement.className = "block w-full h-full";
        container.appendChild(renderer.domElement);

        // the camera looks from the front right like Blender's default view, z up
        const scene = new THREE.Scene();
        const camera = new THREE.PerspectiveCamera(40, 1, 0.1, 1000);
        camera.up.set(0, 0, 1);
        camera.position.set(14, -14, 10);
        const controls = new OrbitControls(camera, renderer.domElement);
        controls.mouseButtons = { LEFT: THREE.MOUSE.ROTATE, MIDDLE: THREE.MOUSE.ROTATE, RIGHT: THREE.MOUSE.PAN };
        controls.update();

        // soft light all round and a sun from the camera's side, so each face is its own shade
        scene.add(new THREE.HemisphereLight(0xffffff, 0x666666, 2));
        const sun = new THREE.DirectionalLight(0xffffff, 1.5);
        sun.position.set(4, -6, 10);
        scene.add(sun);

        // the floor: a grid and the x and y axes through the middle, flat on z
        const floor = new THREE.Group();
        scene.add(floor);
        const gridMaterial = new THREE.LineBasicMaterial();
        const gridPoints: number[] = [];
        const half = GRID_SIZE / 2;
        for (let i = 0; i <= GRID_DIVISIONS; i++) {
            const at = -half + (i * GRID_SIZE) / GRID_DIVISIONS;
            if (Math.abs(at) < 1e-9) continue;
            gridPoints.push(-half, at, 0, half, at, 0, at, -half, 0, at, half, 0);
        }
        const grid = new THREE.LineSegments(new THREE.BufferGeometry().setAttribute("position", new THREE.Float32BufferAttribute(gridPoints, 3)), gridMaterial);
        floor.add(grid);
        const axis = (to: [number, number, number], color: string) => new THREE.Line(new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(...to).multiplyScalar(-half), new THREE.Vector3(...to).multiplyScalar(half)]), new THREE.LineBasicMaterial({ color }));
        floor.add(axis([1, 0, 0], AXIS_COLORS.X), axis([0, 1, 0], AXIS_COLORS.Y));

        // the object the curves move, its shape's meshes go in it (below). blender's xyz euler turns around x first
        const surface = new THREE.MeshStandardMaterial({ roughness: 0.8 });
        const edge = new THREE.LineBasicMaterial();
        const object = new THREE.Group();
        object.rotation.order = "ZYX";
        scene.add(object);

        // the theme's colors, read again when it changes
        const applyTheme = () => {
            const style = getComputedStyle(container);
            const color = (name: string) => new THREE.Color(style.getPropertyValue(name).trim() || "#000");
            scene.background = color("--graph-background");
            gridMaterial.color = color("--graph-grid");
            surface.color = color("--preview-object");
            edge.color = color("--preview-edge");
        };
        applyTheme();

        const render = () => renderer.render(scene, camera);
        view.current = { render, object, floor, surface, edge };
        controls.addEventListener("change", render);

        // sized to the container, drawn right away so a window drag shows each step
        const resize = new ResizeObserver(() => {
            const width = Math.max(container.clientWidth, 1);
            const height = Math.max(container.clientHeight, 1);
            renderer.setSize(width, height, false);
            camera.aspect = width / height;
            camera.updateProjectionMatrix();
            render();
        });
        resize.observe(container);
        // the theme's css is swapped in the head (utils/theme.ts)
        const theme = new MutationObserver(() => {
            applyTheme();
            render();
        });
        theme.observe(document.head, { childList: true, subtree: true, characterData: true });
        const unlisten = listen("settings_changed", () => {
            applyTheme();
            render();
        });

        return () => {
            resize.disconnect();
            theme.disconnect();
            unlisten.then((f) => f());
            controls.dispose();
            scene.traverse((object) => {
                if (object instanceof THREE.Mesh || object instanceof THREE.Line) {
                    object.geometry.dispose();
                    object.material.dispose();
                }
            });
            renderer.dispose();
            renderer.domElement.remove();
            view.current = null;
        };
    }, []);

    // MARK: Shape

    // the shape's meshes with their sharp edges drawn, swapped when it changes
    useEffect(() => {
        const current = view.current;
        if (!current) return;
        const { object, surface, edge } = current;
        for (const geometry of shapeGeometries(shape)) {
            const mesh = new THREE.Mesh(geometry, surface);
            mesh.add(new THREE.LineSegments(new THREE.EdgesGeometry(geometry, EDGE_ANGLE), edge));
            object.add(mesh);
        }
        current.render();
        return () => {
            // the materials stay, they're the scene's
            object.traverse((child) => {
                if (child instanceof THREE.Mesh || child instanceof THREE.LineSegments) child.geometry.dispose();
            });
            object.clear();
        };
    }, [shape]);

    // the floor shown or not
    useEffect(() => {
        const current = view.current;
        if (!current) return;
        current.floor.visible = showGrid;
        current.render();
    }, [showGrid]);

    // MARK: Transform

    // the shape where the curves have it at the playhead
    useEffect(() => {
        const current = view.current;
        if (!current) return;
        const transform = transformAt(channels, time);
        const { object } = current;
        object.position.set(transform.location[0], transform.location[1], transform.location[2]);
        object.scale.set(transform.scale[0], transform.scale[1], transform.scale[2]);
        if (transform.quaternion) {
            // blender's quaternions are w, x, y, z
            const [w, x, y, z] = transform.rotation_quaternion;
            object.quaternion.set(x, y, z, w).normalize();
        } else {
            object.rotation.set(transform.rotation_euler[0], transform.rotation_euler[1], transform.rotation_euler[2]);
        }
        current.render();
    }, [channels, time]);

    return <div ref={containerRef} className="w-full h-full overflow-hidden" />;
}

export default ScenePreview;
