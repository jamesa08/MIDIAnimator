import React, { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';


interface PanelProps {
  id: string;
  name: string;
}

declare global {
  interface Window {
    __TAURI__: any;
  }
}

const WebviewWindow = window.__TAURI__.webviewWindow.WebviewWindow;

const Panel: React.FC<PanelProps> = ({ id, name }) => {
  const navigate = useNavigate();

  useEffect(() => {
    const handleClick = (event: any) => {
      console.log(`Got ${JSON.stringify(event)} on window listener`);
    };

    const setupListener = async () => {
      try {
        const unlisten = await window.__TAURI__.event.listen('clicked', handleClick);
        return () => {  
          unlisten();
        };
      } catch (error) {
        console.error('Failed to setup event listener:', error);
      }
    };

    setupListener();
  }, []);

  const createWindow = (event: React.MouseEvent<HTMLButtonElement>) => {
    const webview = new WebviewWindow(id, {
      url: `/#/panel/${id}`,
      title: name,
      width: 400,
      height: 300,
      resizable: true,
      x: event.screenX,
      y: event.screenY,
    });

    webview.once('tauri://created', () => {
      console.log('Created new window');
    });

    webview.once('tauri://error', (e: any) => {
      console.error(`Error creating new window ${e.payload}`);
    });

    navigate(`/#/panel/${id}`);
  };

  return (
    <div className="panel w-60 select-none">
      <div className="panel-header h-8 border-b border-black flex items-center pl-2 pr-2">
        {name}
      </div>
      <button onClick={createWindow}>Create Window</button>
    </div>
  );
};

export default Panel;