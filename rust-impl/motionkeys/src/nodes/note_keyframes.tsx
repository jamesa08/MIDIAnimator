import SpecNode from "./_SpecNode";

function note_keyframes({ data }: { id: any; data: any }) {
    return <SpecNode nodeType="note_keyframes" data={data} />;
}

export default note_keyframes;
