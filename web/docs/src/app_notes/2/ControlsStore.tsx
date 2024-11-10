import { Provider } from "jotai";

type ControlsStoreProps = {
  children: React.ReactNode;
};

export const ControlsStore = ({ children }: ControlsStoreProps) => (
  <Provider>{children}</Provider>
);

export default ControlsStore;
