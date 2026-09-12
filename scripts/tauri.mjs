import { spawn } from "node:child_process";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

const environment = { ...process.env };

// MNN bundled by ocr-rs conflicts with GCC 16's duplicate __int128
// compatibility declarations on Linux. Keep the workaround local to
// this native build command and preserve any user-provided flags.
if (process.platform === "linux") {
  const workaround = "-U__SIZEOF_INT128__";
  const flags = environment.CXXFLAGS?.split(/\s+/).filter(Boolean) ?? [];
  if (!flags.includes(workaround)) flags.push(workaround);
  environment.CXXFLAGS = flags.join(" ");
}

// Invoke the CLI entry point with Node instead of spawning the platform shim.
// In particular, Node 22 can reject direct `.cmd` execution on Windows with
// `spawn EINVAL`.
const tauriCli = require.resolve("@tauri-apps/cli/tauri.js");
const child = spawn(process.execPath, [tauriCli, ...process.argv.slice(2)], {
  env: environment,
  stdio: "inherit",
});

child.on("error", (error) => {
  console.error("Could not start Tauri CLI: " + error.message);
  process.exitCode = 1;
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exitCode = code ?? 1;
  }
});
