import React from "react";
import { useParams } from "react-router-dom";

const PanelContent: React.FC = () => {
    const { id } = useParams<{ id: string }>();

    console.log("PanelContent component rendered");

    return (
        <div>
            <h1>Hello World {id}</h1>
        </div>
    );
};

export default PanelContent;
