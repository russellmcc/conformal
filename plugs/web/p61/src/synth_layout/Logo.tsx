const subtitleLine = (text: string, idx: number) => (
  <div className="text-border relative" key={idx}>
    <div className="absolute inset-0 font-light italic blur-[1px]">{text}</div>
    <div className="font-light italic">{text}</div>
  </div>
);

const title = (text: string) => (
  <div className={"text-pop relative"}>
    <div
      className="text-logo flex h-[47px] items-center align-middle font-bold blur-[2px]"
      style={{
        WebkitTextStroke: `1px`,
        WebkitTextFillColor: "transparent",
      }}
    >
      {text}
    </div>

    <div
      className="text-logo absolute inset-0 flex items-center align-middle font-bold"
      style={{
        WebkitTextStroke: `1px`,
        WebkitTextFillColor: "transparent",
      }}
    >
      {text}
    </div>
  </div>
);

export const Logo = () => (
  <div className="flex h-[47px] w-[395px] cursor-default select-none flex-row justify-end">
    {title("POLY 81")}
    <div className="text-border ml-[5px] mr-[37px] flex flex-col justify-around pb-[3px] leading-3">
      {["an open-source", "virtual analogue", "synthesizer"].map(subtitleLine)}
    </div>
  </div>
);

export default Logo;
