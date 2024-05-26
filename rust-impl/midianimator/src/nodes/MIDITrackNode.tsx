import { useCallback, useState } from 'react';
import { Handle, Position } from 'reactflow';
import { open } from '@tauri-apps/plugin-dialog';


const handleStyle = { left: 10};

function MIDITrackNode({ data, isConnectable }: { data: any, isConnectable: any }) {
    const onChange = useCallback((evt: any) => {
        console.log(evt.target.value);
    }, []);

    const pick = useCallback(async () => {
        let res = await onMIDIFilePick();
        if (res != null) { setFile(res.toString()); }
    }, []);

    const [file, setFile] = useState('');

    return (
        <div className="midi-track-node">
          <Handle type="target" position={Position.Top} isConnectable={isConnectable} />
          <div>
            <label htmlFor="text">MIDI File:</label>
            <button onClick={pick}>Pick MIDI File</button>
            <div>{file}</div>
          </div>
          <Handle
            type="source"
            position={Position.Bottom}
            id="a"
            style={handleStyle}
            isConnectable={isConnectable}
          />
          <Handle type="source" position={Position.Bottom} id="b" isConnectable={isConnectable} />
        </div>
      );
}

async function onMIDIFilePick() {
    const selected = await open({
        multiple: false,
        filters: [{
          name: 'Image',
          extensions: ['mid', 'midi']
        }]
      });
    
    return selected;
}

export default MIDITrackNode;