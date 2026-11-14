// Spike S4: materialize a pack from a git snapshot into a content-keyed cache, verify it against the lock,
// and link the bundled SDK. Stand-in for what Collide's host would do in Rust.
//
//   node lib/pack-cache.mjs tree        --repo R --ref REF --path P
//   node lib/pack-cache.mjs hash-dir    --dir D [--algo sha1|sha256]
//   node lib/pack-cache.mjs materialize --repo R --ref REF --path P --lock TREE --cache C
//                                       [--method raw|archive-tree|archive-commit]
//                                       [--sdk DIR --link symlink|copy] [--no-package-json] [--writable]
//
// Prints one JSON object on stdout. Exit 1 with {"ok":false,...} on any refusal.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync, cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, renameSync,
  rmSync, symlinkSync, writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const GIT_ENV = { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" };

export function git(repo, args, input) {
  const result = spawnSync("git", ["-C", repo, ...args], {
    env: GIT_ENV, input, maxBuffer: 1 << 30,
  });
  if (result.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed (${result.status}): ${result.stderr.toString().trim()}`);
  }
  return result.stdout;
}

export function objectFormat(repo) {
  return git(repo, ["rev-parse", "--show-object-format"]).toString().trim();
}

export function resolveTree(repo, ref, path) {
  if (ref.startsWith("-")) throw new Error(`refused: ref ${ref} looks like an option`);
  const id = git(repo, ["rev-parse", "--verify", `${ref}:${path}`]).toString().trim();
  const type = git(repo, ["cat-file", "-t", id]).toString().trim();
  if (type !== "tree") throw new Error(`${ref}:${path} is a ${type}, not a directory`);
  return id;
}

function hashObject(type, body, algo) {
  return createHash(algo).update(`${type} ${body.length}\0`).update(body).digest("hex");
}

// Git orders tree entries by name bytes, comparing a tree's name as if it ended in "/".
function compareEntries(a, b) {
  const left = Buffer.from(a.name + (a.mode === "40000" ? "/" : ""));
  const right = Buffer.from(b.name + (b.mode === "40000" ? "/" : ""));
  return Buffer.compare(left, right);
}

/** Git tree id of a directory on disk, computed without git. Empty directories are skipped, as in git. */
export function hashDir(dir, algo = "sha1", exclude = new Set()) {
  const entries = [];
  for (const name of readdirSync(dir)) {
    if (exclude.has(name)) continue;
    const full = join(dir, name);
    const stat = lstatSync(full);
    if (stat.isDirectory()) {
      const id = hashDir(full, algo);
      if (id !== null) entries.push({ mode: "40000", name, id });
    } else if (stat.isSymbolicLink()) {
      entries.push({ mode: "120000", name, id: hashObject("blob", Buffer.from(readlinkSync(full)), algo) });
    } else if (stat.isFile()) {
      const mode = stat.mode & 0o100 ? "100755" : "100644";
      entries.push({ mode, name, id: hashObject("blob", readFileSync(full), algo) });
    } else {
      throw new Error(`unsupported file type in pack: ${full}`);
    }
  }
  if (entries.length === 0) return null;
  entries.sort(compareEntries);
  const body = Buffer.concat(entries.flatMap((entry) => [
    Buffer.from(`${entry.mode} ${entry.name}\0`), Buffer.from(entry.id, "hex"),
  ]));
  return hashObject("tree", body, algo);
}

function parseLsTree(buffer) {
  const entries = [];
  for (const record of buffer.toString("utf8").split("\0")) {
    if (record === "") continue;
    const tab = record.indexOf("\t");
    const [mode, type, oid] = record.slice(0, tab).split(" ");
    entries.push({ mode, type, oid, path: record.slice(tab + 1) });
  }
  return entries;
}

function readBlobs(repo, oids) {
  const output = git(repo, ["cat-file", "--batch"], oids.map((oid) => `${oid}\n`).join(""));
  const blobs = new Map();
  let offset = 0;
  while (offset < output.length) {
    const newline = output.indexOf(0x0a, offset);
    const [oid, type, size] = output.subarray(offset, newline).toString().split(" ");
    if (type !== "blob") throw new Error(`expected a blob for ${oid}, got ${type}`);
    const start = newline + 1;
    blobs.set(oid, output.subarray(start, start + Number(size)));
    offset = start + Number(size) + 1;
  }
  return blobs;
}

/** Method "raw": read the tree's objects directly. No attributes, filters or eol conversion apply. */
export function extractRaw(repo, tree, dest) {
  const entries = parseLsTree(git(repo, ["ls-tree", "-r", "-z", tree]));
  for (const entry of entries) {
    if (entry.mode === "120000") {
      const target = readBlobs(repo, [entry.oid]).get(entry.oid).toString();
      throw new Error(`refused: the pack contains a symlink, ${entry.path} -> ${target}`);
    }
    if (entry.mode === "160000") throw new Error(`refused: the pack contains a submodule at ${entry.path}`);
  }
  const blobs = readBlobs(repo, [...new Set(entries.map((entry) => entry.oid))]);
  for (const entry of entries) {
    const file = join(dest, entry.path);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, blobs.get(entry.oid), { mode: entry.mode === "100755" ? 0o755 : 0o644 });
  }
  return entries.length;
}

/** Methods "archive-tree" (`git archive REF:PATH`) and "archive-commit" (`git archive REF -- PATH`). */
export function extractArchive(repo, ref, path, dest, method) {
  const args = method === "archive-tree"
    ? ["archive", "--format=tar", `${ref}:${path}`]
    : ["archive", "--format=tar", ref, "--", path];
  const tar = git(repo, args);
  const depth = method === "archive-tree" ? 0 : path.split("/").filter(Boolean).length;
  const result = spawnSync("tar", ["-x", "-f", "-", "-C", dest, `--strip-components=${depth}`], { input: tar });
  if (result.status !== 0) throw new Error(`tar failed: ${result.stderr.toString().trim()}`);
  return Number(spawnSync("tar", ["-t", "-f", "-"], { input: tar }).stdout.toString().split("\n")
    .filter((line) => line && !line.endsWith("/")).length);
}

export function linkSdk(root, sdkDir, mode) {
  const target = join(root, "node_modules", "@collide", "pack-sdk");
  mkdirSync(dirname(target), { recursive: true });
  if (mode === "symlink") symlinkSync(sdkDir, target, "dir");
  else if (mode === "copy") cpSync(sdkDir, target, { recursive: true });
  else throw new Error(`unknown link mode ${mode}`);
}

function setWritable(dir, writable) {
  const apply = (path, mode) => chmodSync(path, writable ? mode | 0o200 : mode & ~0o222);
  if (writable) apply(dir, lstatSync(dir).mode);
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    const stat = lstatSync(full);
    if (stat.isDirectory()) setWritable(full, writable);
    else apply(full, stat.mode);
  }
  if (!writable) apply(dir, lstatSync(dir).mode);
}

export function materialize(options) {
  const timings = {};
  const started = performance.now();
  const mark = (name, since) => { timings[name] = Number((performance.now() - since).toFixed(2)); };

  let t = performance.now();
  const algo = objectFormat(options.repo);
  const tree = resolveTree(options.repo, options.ref, options.path);
  mark("resolveMs", t);
  if (tree !== options.lock) {
    throw Object.assign(new Error(`lock mismatch: ${options.ref}:${options.path} is ${tree}, the lock says ${options.lock}`),
      { stage: "lock" });
  }

  const packsDir = join(options.cache, "packs");
  const dest = join(packsDir, tree);
  if (existsSync(join(dest, ".collide-verified"))) {
    timings.totalMs = Number((performance.now() - started).toFixed(2));
    return { ok: true, cache: "hit", tree, dest, timings };
  }

  const staging = join(packsDir, `.staging-${tree}-${process.pid}`);
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(join(staging, "pack"), { recursive: true });
  try {
    t = performance.now();
    const method = options.method ?? "raw";
    const files = method === "raw"
      ? extractRaw(options.repo, tree, join(staging, "pack"))
      : extractArchive(options.repo, options.ref, options.path, join(staging, "pack"), method);
    mark("extractMs", t);

    t = performance.now();
    const onDisk = hashDir(join(staging, "pack"), algo);
    mark("verifyMs", t);
    if (onDisk !== tree) {
      throw Object.assign(new Error(`extracted tree hashes to ${onDisk}, expected ${tree}`), { stage: "verify", files });
    }

    t = performance.now();
    if (!options.noPackageJson) {
      writeFileSync(join(staging, "package.json"), `${JSON.stringify({ private: true, type: "module" })}\n`);
    }
    if (options.sdk) linkSdk(staging, options.sdk, options.link ?? "symlink");
    writeFileSync(join(staging, ".collide-verified"), `${tree}\n`);
    if (!options.writable) setWritable(join(staging, "pack"), false);
    mark("linkMs", t);

    t = performance.now();
    mkdirSync(packsDir, { recursive: true });
    try {
      renameSync(staging, dest);
    } catch (error) {
      // Another loader published the same tree first. Its copy is verified, so use it.
      if (!existsSync(join(dest, ".collide-verified"))) throw error;
      setWritable(join(staging, "pack"), true);
      rmSync(staging, { recursive: true, force: true });
      timings.totalMs = Number((performance.now() - started).toFixed(2));
      return { ok: true, cache: "lost-race", tree, dest, timings };
    }
    mark("renameMs", t);
    timings.totalMs = Number((performance.now() - started).toFixed(2));
    return { ok: true, cache: "miss", tree, algo, method, files, dest, timings };
  } catch (error) {
    rmSync(staging, { recursive: true, force: true });
    throw error;
  }
}

function timeHashDir(dir, algo, repeat) {
  const times = [];
  let tree;
  for (let i = 0; i < repeat; i += 1) {
    const started = performance.now();
    tree = hashDir(dir, algo);
    times.push(performance.now() - started);
  }
  times.sort((a, b) => a - b);
  const ms = (value) => Number(value.toFixed(3));
  return { tree, runs: repeat, medianMs: ms(times[Math.floor(repeat / 2)]), maxMs: ms(times[repeat - 1]) };
}

function parseArgs(argv) {
  const options = {};
  for (let i = 0; i < argv.length; i += 1) {
    const key = argv[i].replace(/^--/, "").replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (["noPackageJson", "writable"].includes(key)) options[key] = true;
    else { options[key] = argv[i + 1]; i += 1; }
  }
  return options;
}

function main() {
  const [command, ...rest] = process.argv.slice(2);
  const options = parseArgs(rest);
  try {
    let output;
    if (command === "tree") output = { tree: resolveTree(options.repo, options.ref, options.path) };
    else if (command === "hash-dir") output = timeHashDir(options.dir, options.algo ?? "sha1", Number(options.repeat ?? 1));
    else if (command === "materialize") output = materialize(options);
    else throw new Error(`unknown command ${command}`);
    process.stdout.write(`${JSON.stringify(output)}\n`);
  } catch (error) {
    process.stdout.write(`${JSON.stringify({ ok: false, stage: error.stage ?? "error", error: error.message })}\n`);
    process.exitCode = 1;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
