import InterfaceNode from "./_InterfaceNode";

// inside a group: the values the group node outputs
function group_output({ id, data }: { id: any; data: any }) {
    return <InterfaceNode id={id} data={data} side="outputs" />;
}

export default group_output;
