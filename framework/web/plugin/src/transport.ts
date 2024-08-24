export default interface Transport<Request, Response> {
  request: (m: Request) => void;
  setOnResponse: (recv: (m: Response) => void) => void;
}
