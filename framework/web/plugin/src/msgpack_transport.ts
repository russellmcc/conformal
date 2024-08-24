import { decode, encode } from "@msgpack/msgpack";
import { Transport as ProtocolTransport, Request, Response } from "./protocol";
import Transport from "./transport";

const transport = (
  transport: Transport<Uint8Array, Uint8Array>,
): ProtocolTransport => {
  const request = (request: Request) => {
    transport.request(
      encode(request, { forceFloat32: true, forceIntegerToFloat: true }),
    );
  };
  const setOnResponse = (onResponse: (response: Response) => void) => {
    transport.setOnResponse((bytes: Uint8Array) => {
      try {
        onResponse(Response.parse(decode(bytes)));
      } catch {
        // If we couldn't decode the response, it could be from a new server version -
        // so we silently ignore it!
      }
    });
  };
  return {
    request,
    setOnResponse,
  };
};

export default transport;
