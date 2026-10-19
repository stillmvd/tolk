import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const tauri = fileURLToPath(new URL("../src-tauri/", import.meta.url));
const cmake = "C:\\Program Files\\CMake\\bin";
const env = { ...process.env };
if (existsSync(cmake) && !(env.PATH ?? env.Path ?? "").includes(cmake)) env.PATH = `${cmake};${env.PATH ?? env.Path}`;
execFileSync("cargo", ["build", "-p", "tolk-mt", "--release"], { cwd: tauri, stdio: "inherit", env });
mkdirSync(join(tauri, "binaries"), { recursive: true });
copyFileSync(join(tauri, "target", "release", "tolk-mt.exe"), join(tauri, "binaries", "tolk-mt-x86_64-pc-windows-msvc.exe"));
