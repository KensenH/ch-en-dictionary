import { spawn } from "node:child_process";

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

const command = process.platform === "win32" ? "tauri.cmd" : "tauri";
const child = spawn(command, process.argv.slice(2), {
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
