// Temporary workaround for https://github.com/oven-sh/bun/issues/4890
/// <reference lib="dom" />
/// <reference lib="dom.iterable" />
import Transport from "./transport";

// Declare the the special IPC type from Wry.
declare global {
  // eslint-disable-next-line @typescript-eslint/consistent-type-definitions
  interface Window {
    ipc:
      | {
          postMessage: (m: string) => void;
        }
      | undefined;

    // This is a function we add to the window object to receive messages from
    // Wry. It is set by the setOnResponse function below.
    receiveMessage: (m: string) => void;
  }
}

const request = (m: Uint8Array) => {
  window.ipc!.postMessage(window.btoa(String.fromCodePoint(...m)));
};

const setOnResponseInternal = (recv: (m: string) => void) => {
  window.receiveMessage = recv;
};

const setOnResponse = (recv: (m: Uint8Array) => void) => {
  setOnResponseInternal((m) => {
    try {
      // Note that `atob` will give a 'string' that is actually a binary array encoded as codepoints, so this unwrap is safe.
      recv(Uint8Array.from(window.atob(m), (m) => m.codePointAt(0)!));
    } catch (exn) {
      return;
    }
  });
};

const transport: Transport<Uint8Array, Uint8Array> | undefined = window.ipc
  ? {
      request,
      setOnResponse,
    }
  : undefined;

export default transport;
