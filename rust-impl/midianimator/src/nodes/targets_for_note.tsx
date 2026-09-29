import SpecNode from "./_SpecNode";

function targets_for_note({ data }: { id: any; data: any }) {
    return <SpecNode nodeType="targets_for_note" data={data} />;
}

export default targets_for_note;
