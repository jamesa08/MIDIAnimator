import InterfaceNode from "./_InterfaceNode";

// inside a group: the values given to the group node's inputs
function group_input({ id, data }: { id: any; data: any }) {
    return <InterfaceNode id={id} data={data} side="inputs" />;
}

export default group_input;
