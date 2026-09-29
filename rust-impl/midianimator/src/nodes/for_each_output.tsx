import SpecNode from "./_SpecNode";

// end of a for each zone, collects each item's result into a list
function for_each_output({ data }: { id: any; data: any }) {
    return <SpecNode nodeType="for_each_output" data={data} headerType="ZONE" />;
}

export default for_each_output;
