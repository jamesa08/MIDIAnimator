import Tool from "./Tool";
import { useStateContext } from "../contexts/StateContext";

function MenuBar() {
    const { frontEndState, setFrontEndState } = useStateContext();

    // shows or hides everything docked on a side
    const collapse = (side: "left" | "right") => {
        setFrontEndState((prev: any) => ({
            ...prev,
            sidesHidden: prev.sidesHidden.includes(side) ? prev.sidesHidden.filter((s: string) => s !== side) : [...prev.sidesHidden, side],
        }));
    };
    const collapseLeft = () => collapse("left");
    const collapseRight = () => collapse("right");

    return (
        <div className="toolbar flex h-8 items-center pr-[3px]">
            {/* logo */}
            <div className="logo flex justify-center px-1">
                <img src="logo.webp" alt="logo" className="h-8 py-1" />
            </div>

            <div className="spacer h-5 w-[1px] bg-black mr-1" />

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
