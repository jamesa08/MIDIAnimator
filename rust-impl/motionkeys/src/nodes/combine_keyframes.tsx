import SpecNode from "./_SpecNode";

function combine_keyframes({ data }: { id: any; data: any }) {
    return <SpecNode nodeType="combine_keyframes" data={data} />;
}

export default combine_keyframes;
