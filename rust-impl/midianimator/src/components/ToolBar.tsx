import Tool from "./Tool";
import { useStateContext } from "../contexts/StateContext";

function MenuBar() {
    const { frontEndState, setFrontEndState } = useStateContext();

    const collapseLeft = () => {
        setFrontEndState((prev: any) => ({
            ...prev,
            panelsShown: prev.panelsShown.includes(0)
                ? prev.panelsShown.filter((id: number) => id !== 0)
                : [...prev.panelsShown, 0],
        }));
    };

    const collapseRight = () => {
        setFrontEndState((prev: any) => ({
            ...prev,
            panelsShown: prev.panelsShown.includes(1)
                ? prev.panelsShown.filter((id: number) => id !== 1)
                : [...prev.panelsShown, 1],
        }));
    };

    return (
        <div className="toolbar border-b border-b-black flex h-8 items-center pr-1">
            {/* logo */}
            <div className="logo w-[71px] flex justify-center">
                <img src="src/logo.png" alt="logo" width="90%" className="p-1" />
            </div>

            <div className="spacer h-[inherit] w-[1px] bg-black mr-1" />

            {/* left aligned items */}
            <div className="float-left inline-flex">
                <Tool type="collapse-left" onClick={collapseLeft} />
                <Tool type="save" />
                <Tool type="load" />
            </div>

            {/* other icons here */}

            {/* right aligned items */}
            <div className="ml-auto flex">
                <Tool type="run" />
                <Tool type="collapse-right" onClick={collapseRight} />
            </div>
        </div>
    );
}

export default MenuBar;
