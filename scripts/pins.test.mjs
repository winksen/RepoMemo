// node --test scripts/pins.test.mjs  (npm run pins:test)
import assert from "node:assert/strict";
import { test } from "node:test";
import { pinnedImageNames, pinProblems, unpinnedActions, unpinnedFroms, unpinnedImages } from "./pins.mjs";

const sha = "a".repeat(40);
const digest = `sha256:${"b".repeat(64)}`;

test("an action must name a commit", () => {
  assert.deepEqual(unpinnedActions([["a.yml", `      - uses: actions/checkout@${sha} # v4`]]), []);
  assert.equal(unpinnedActions([["a.yml", "      - uses: actions/checkout@v4"]]).length, 1);
  assert.equal(unpinnedActions([["a.yml", "      - uses: actions/checkout@main"]]).length, 1);
  assert.equal(unpinnedActions([["a.yml", "      - uses: docker/build-push-action"]]).length, 1);
  assert.deepEqual(unpinnedActions([["a.yml", "      - uses: ./.github/actions/mine"]]), []);
});

test("a base image must name a digest, a stage and scratch need none, and a platform flag is not the image", () => {
  const good = `FROM node:22@${digest} AS web\nFROM scratch\nFROM web AS next\nFROM --platform=linux/amd64 debian:trixie@${digest} AS server\n`;
  assert.deepEqual(unpinnedFroms(good), []);
  assert.equal(unpinnedFroms("FROM node:22 AS web\n").length, 1);
  assert.equal(unpinnedFroms("FROM --platform=linux/amd64 debian:trixie AS server\n").length, 1);
});

test("an image pinned anywhere is pinned everywhere", () => {
  const pinned = ["Dockerfile", `FROM caddy:2-alpine@${digest} AS edge`];
  assert.deepEqual([...pinnedImageNames([pinned])], ["caddy"]);
  assert.deepEqual(unpinnedImages([pinned, ["ci.yml", `docker run caddy:2-alpine@${digest} caddy validate`]]), []);
  const moving = unpinnedImages([pinned, ["ci.yml", "docker run --rm caddy:2-alpine caddy validate"]]);
  assert.equal(moving.length, 1);
  assert.match(moving[0], /ci\.yml names caddy:2-alpine/);
  // A tag that merely starts like a pinned one is still a tag with no digest.
  assert.equal(unpinnedImages([pinned, ["compose.yml", "image: caddy:2"]]).length, 1);
  // The project's own images and a registry host in a path are other names.
  assert.deepEqual(unpinnedImages([pinned, ["compose.yml", "image: repomemo-caddy:2\nimage: ghcr.io/me/caddy:2"]]), []);
});

test("a registry path is a name of its own", () => {
  const files = [["ci.yml", `TRIVY_IMAGE: ghcr.io/aquasecurity/trivy:0.75.0@${digest}`], ["x.yml", "docker run ghcr.io/aquasecurity/trivy:0.75.0 image"]];
  assert.equal(unpinnedImages(files).length, 1);
});

test("the project's own files pass together", () => {
  const problems = pinProblems({
    workflows: [["ci.yml", `- uses: actions/checkout@${sha}\n  run: docker run prom/prometheus:v3@${digest}`]],
    dockerfile: `FROM debian:trixie-slim@${digest} AS server\n`,
    others: [["docker-compose.yml", `image: prom/prometheus:v3@${digest}`]],
  });
  assert.deepEqual(problems, []);
});
