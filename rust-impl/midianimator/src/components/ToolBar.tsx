import Tool from "./Tool";

function MenuBar() {
    return (
        <div className="toolbar border-b border-b-black flex h-8 items-center pr-1">
            {/* logo */}
            <div className="logo w-[71px] flex justify-center">
                <img src="src/logo.png" alt="logo" width="90%" className="p-1" />
            </div>

            <div className="spacer h-[inherit] w-[1px] bg-black mr-1" />

            {/* left aligned items */}
            <div className="float-left">
                <Tool type="collapse-left" />
            </div>

            {/* other icons here */}
            
            {/* right aligned items */}
            <div className="ml-auto flex">
                <Tool type="run" />
                <Tool type="collapse-right" />
            </div>
        </div>
    );
}

export default MenuBar;
