import SpecNode from "./_SpecNode";

// start of a for each zone, the nodes between it and its paired for each output run once per item
function for_each_input({ data }: { id: any; data: any }) {
    return <SpecNode nodeType="for_each_input" data={data} />;
}

export default for_each_input;
