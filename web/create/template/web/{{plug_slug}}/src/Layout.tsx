import { useNumericParam } from "@conformal/plugin";

const Layout = () => {
  const { value: gain, set: setGain } = useNumericParam("gain");

  return (
    <div>
      <p>Current gain: {gain}%</p>
      <p>
        <span
          onClick={() => {
            setGain(Math.max(0, gain - 10));
          }}
        >
          -
        </span>
        <span
          onClick={() => {
            setGain(Math.min(100, gain + 10));
          }}
        >
          +
        </span>
      </p>
    </div>
  );
};

export default Layout;
