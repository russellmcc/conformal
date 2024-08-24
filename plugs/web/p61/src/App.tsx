import { DevModeTools } from "plugin";
import SynthLayout from "./synth_layout";

const App = () => (
  <div className="bg-backg h-full">
    <div className="flex h-full flex-row items-center justify-around">
      <SynthLayout />
    </div>
    <DevModeTools />
  </div>
);

export default App;
