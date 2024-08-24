import IgnoreErrorBoundary from "./IgnoreErrorBoundary";
import { useBooleanAtom, useBooleanValue } from "plugin";

const DevModeToolsInternal = () => {
  const isDevMode = useBooleanValue("prefs/dev_mode");
  const [webDevServer, setWebDevServer] = useBooleanAtom(
    "prefs/use_web_dev_server",
  );

  if (!isDevMode) {
    return <></>;
  }
  return (
    <div
      style={{
        backgroundColor: "black",
        padding: "5px",
        position: "absolute",
        bottom: "0",
        right: "3px",
        cursor: "pointer",
        userSelect: "none",
        WebkitUserSelect: "none",
        fontSize: "8px",
        color: "white",
      }}
      onClick={() => {
        setWebDevServer(!webDevServer);
      }}
    >
      {webDevServer ? "switch to embedded" : "switch to dev server"}
    </div>
  );
};

const DevModeTools = () => (
  <IgnoreErrorBoundary>
    <DevModeToolsInternal />
  </IgnoreErrorBoundary>
);

export default DevModeTools;
