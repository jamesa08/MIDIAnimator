import SpecNode from "./_SpecNode";

function note_targets({ data }: { id: any; data: any }) {
    return <SpecNode nodeType="note_targets" data={data} />;
}

export default note_targets;
