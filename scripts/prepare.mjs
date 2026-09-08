import { spawnSync } from "node:child_process";
import { realpathSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = realpathSync(fileURLToPath(new URL("../", import.meta.url)));
const repository = spawnSync("git", ["rev-parse", "--show-toplevel"], {
  cwd: root,
  encoding: "utf8",
  windowsHide: true,
});

function run(executable, arguments_) {
  const result = spawnSync(executable, arguments_, {
    cwd: root,
    stdio: "inherit",
    windowsHide: true,
  });

  if (result.error) console.error(result.error.message);
  if (result.status !== 0) process.exit(result.status ?? 1);
}

if (repository.status === 0 && realpathSync(repository.stdout.trim()) === root) {
  run("git", ["config", "--local", "core.hooksPath", ".githooks"]);
}

run(process.execPath, [path.join(root, "tools.mjs"), "install"]);
