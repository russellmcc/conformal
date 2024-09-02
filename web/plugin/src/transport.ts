type Transport<Request, Response> = {
  request: (m: Request) => void;
  setOnResponse: (recv: (m: Response) => void) => void;
};
export default Transport;
