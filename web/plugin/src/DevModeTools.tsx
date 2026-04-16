import { useEffect } from "react";
import IgnoreErrorBoundary from "./IgnoreErrorBoundary";
import { useBooleanAtom, useBooleanValue } from "./stores_react";

const DevModeToolsInternal = () => {
  const isDevMode = useBooleanValue("prefs/dev_mode");
  const [webDevServer, setWebDevServer] = useBooleanAtom(
    "prefs/use_web_dev_server",
  );

  useEffect(() => {
    console.warn("isDevMode", isDevMode);
    if (isDevMode) {
      return;
    }

    const disableDefaultContextMenu = (event: MouseEvent) => {
      // Run in the bubbling phase so any custom context menu handler sees the
      // original event before we suppress the browser's built-in menu.
      event.preventDefault();
    };

    window.addEventListener("contextmenu", disableDefaultContextMenu);
    return () => {
      window.removeEventListener("contextmenu", disableDefaultContextMenu);
    };
  }, [isDevMode]);

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

/**
 * Component that displays developer tools if the `dev_mode` preference is enabled.
 *
 * This also _disables_ the default context menus when devmode is disabled.
 *
 * This includes for example, a toggle to render the UI embedded in the plug-in or the dev server UI.
 *
 * @group Components
 */
const DevModeTools = () => (
  <IgnoreErrorBoundary>
    <DevModeToolsInternal />
  </IgnoreErrorBoundary>
);

export default DevModeTools;
