const uuidHex = () =>
  crypto.randomUUID().replace(/-/g, "").replace(/../g, "0x$&, ").slice(0, -1);

export default uuidHex;
