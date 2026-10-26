#!/usr/bin/env node
// Reads `docker compose config --format json` on stdin and checks what the stack would run (V0.4.40).
//
//   docker compose config --format json | node scripts/check-compose-images.mjs released 9.9.9 ghcr.io/repomemo
//   docker compose config --format json | node scripts/check-compose-images.mjs built 9.9.9
//
// "released": every image is <registry>/repomemo-<name>:<version> and no service has a build, so `docker compose pull && up`
// needs no compiler. "built": every service of the four images has a build (docker-compose.build.yml) and a local image name.

import { readFileSync } from "node:fs";

const [mode, version, registry] = process.argv.slice(2);
const config = JSON.parse(readFileSync(0, "utf8"));
const problems = [];

for (const name of ["server", "converter", "web", "edge"]) {
  const service = config.services?.[name];
  if (!service) {
    problems.push(`the stack has no ${name} service`);
    continue;
  }
  if (mode === "released") {
    const wanted = `${registry}/repomemo-${name}:${version}`;
    if (service.image !== wanted) problems.push(`${name} runs ${service.image}, not ${wanted}`);
    if (service.build) problems.push(`${name} has a build; the released stack builds nothing`);
  } else if (mode === "built") {
    if (!service.build) problems.push(`${name} has no build`);
    if (service.image !== `repomemo-${name}:${version}`) problems.push(`${name} would be tagged ${service.image}`);
  } else {
    problems.push(`unknown mode ${mode}`);
  }
}

if (problems.length > 0) {
  console.error(problems.map((problem) => `compose images: ${problem}`).join("\n"));
  process.exit(1);
}
console.log(`compose images: ${mode} stack is as expected for ${version}`);
