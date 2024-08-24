import Dco1 from "./Dco1";
import Dco2 from "./Dco2";
import Env from "./Env";
import Logo from "./Logo";
import Mg from "./Mg";
import Vca from "./Vca";
import Vcf from "./Vcf";

export const SynthLayout = () => (
  <div className="select-none">
    <div className="flex items-start">
      <Dco1 />
      <Dco2 />
      <Env />
    </div>
    <div className="mt-[-207px] flex items-start">
      <Mg />
      <Logo />
    </div>
    <div className="ml-[166px] mt-[-159px] flex items-start">
      <Vcf />
      <Vca />
    </div>
  </div>
);

export default SynthLayout;
