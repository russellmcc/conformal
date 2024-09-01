import { promises as fs } from "node:fs";
import path from "node:path";
import { compile } from "handlebars";

const dirExists = async (dir: string) => {
  try {
    await fs.readdir(dir);
    return true;
  } catch {
    return false;
  }
};

export const stampTemplate = async (
  dest: string,
  templateDir: string,
  env: Record<string, string>,
) => {
  if (await dirExists(dest)) {
    throw new Error(`Directory already exists: ${dest}`);
  }

  const files = await fs.readdir(templateDir, {
    recursive: true,
    withFileTypes: true,
  });
  for (const file of files) {
    const srcPath = path.join(file.path, file.name);
    const destPath = path.join(
      dest,
      compile(path.relative(templateDir, srcPath))(env),
    );

    if (file.isDirectory()) {
      continue;
    }

    if (await Bun.file(destPath).exists()) {
      throw new Error(`File already exists: ${destPath}`);
    }

    await fs.mkdir(path.dirname(destPath), { recursive: true });
    const srcContent = await Bun.file(srcPath).text();
    await Bun.write(destPath, compile(srcContent)(env));
  }
};
