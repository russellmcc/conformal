import * as fs from "node:fs/promises";

// Restore from backup
await fs.rename("./package.json.bak", "./package.json");
