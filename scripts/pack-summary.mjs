#!/usr/bin/env node
// One line about an `npm pack --json` result: the packed and installed size of the one package.
import { readFileSync } from "node:fs";

const [p] = JSON.parse(readFileSync(process.argv[2], "utf8"));
console.log(
  `npm tarball: ${p.size} bytes packed, ${p.unpackedSize} unpacked, ${p.files.length} files`,
);
