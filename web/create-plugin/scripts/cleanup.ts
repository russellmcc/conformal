import path from "node:path";
import { unlink } from "node:fs/promises";

await unlink(path.join(__dirname, "..", "rust_versions.toml"));
