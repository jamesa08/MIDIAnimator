import logo from './logo.svg';
import './App.css';

import React, { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { open } from '@tauri-apps/api/dialog';
import ReactFlow, {
    MiniMap,
    Controls,
    Background,
    useNodesState,
    useEdgesState,
    addEdge,
    Connection,
    Edge,
    BackgroundVariant,
    Position
  } from 'reactflow';
  
import 'reactflow/dist/style.css';
import MIDITrackNode from './nodes/MIDITrackNode';

const nodeDefaults = {
    sourcePosition: Position.Right,
    targetPosition: Position.Left,
  };

const initialNodes = [
    { id: '1', position: { x: 0, y: 0 }, data: { label: "Track" }, type: "midiTrack" },
    { id: '2', position: { x: 0, y: 100 }, data: { label: '2' }, ...nodeDefaults},
];
const initialEdges = [{ id: 'e1-2', source: '1', target: '2' }];

const nodeTypes = { "midiTrack": MIDITrackNode };


function App() {
    const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
    const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

    const onConnect = useCallback(
        (params: Edge | Connection) => setEdges((eds) => addEdge(params, eds)),
        [setEdges],
    );
    // const [nodeName, setNodeName] = useState('Node 1');


    const [message, setMessage] = useState('');
    const [name, setName] = useState('');
    const [age, setAge] = useState('');

    const handleGreet = async () => {
        const selected = await open({
            multiple: false,
            filters: [{
              name: 'Image',
              extensions: ['mid', 'midi']
            }]
          });
          try {
            setMessage(JSON.stringify(selected));
            // const greetMessage = await invoke('process_input', { name, age: parseInt(age) });
            // setMessage(greetMessage as string);
            // // setNodeName(greetMessage as string);
        } catch (error) {
            console.error(error);
            // Handle the error
        }
    };
    
    // https://reactflow.dev/examples/nodes/update-node
    // useEffect(() => {
    //     setNodes((nds) => 
    //         nds.map((node) => {
    //             if (node.id === '1') {
    //                 node.data = {
    //                     ...node.data,
    //                     label: nodeName,
    //                 };
    //             }

    //             return node;
    //         })
    //     );
    // }, [nodeName, setNodes]);

    return (
    <div className="App">
        <header className="App-header">
        {/* <img src={logo} className="App-logo" alt="logo" /> */}
        <div>
            <label htmlFor="nameInput">Name: </label>
            <input
            id="nameInput"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            />
        </div>
        <div>
            <label htmlFor="ageInput">Age: </label>
            <input
            id="ageInput"
            type="number"
            value={age}
            onChange={(e) => setAge(e.target.value)}
            />
        </div>
        <button onClick={handleGreet}>Greet</button>
        <div>{message}</div>
        <a
            className="App-link"
            href="https://reactjs.org"
            target="_blank"
            rel="noopener noreferrer"
        >
            Learn React
        </a>
        <div style={{ width: '100vw', height: '50vh' }}>
            <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            nodeTypes={nodeTypes}
            fitView
        >
            <Controls />
            {/* <MiniMap /> */}
            <Background variant={BackgroundVariant.Dots} gap={12} size={1} />
        </ReactFlow>
        </div>

        </header>
    </div>
    );
    }

    export default App;