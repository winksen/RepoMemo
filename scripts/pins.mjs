// The pinning rules (V0.4.42), shared by `container:check` and its tests: nothing that runs in CI or in a container may move under
// us. An action is a commit, a base image in a Dockerfile is a digest, and so is every other image the project names.
//
// Each function takes text and returns the problems it finds, as sentences.

const DIGEST = /@sha256:[0-9a-f]{64}$/;
const escapeRegExp = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** `uses:` lines of workflows ([[file, text]]) that name a tag or a branch instead of a commit. */
export function unpinnedActions(workflows) {
  const problems = [];
  for (const [name, text] of workflows) {
    for (const match of text.matchAll(/^\s*-?\s*uses:\s*(\S+)/gm)) {
      const [action, ref] = match[1].split("@");
      if (action.startsWith("./")) continue;
      if (!/^[0-9a-f]{40}$/.test(ref ?? "")) problems.push(`.github/workflows/${name} uses ${match[1]}, which is a tag or branch; pin it to a commit`);
    }
  }
  return problems;
}

/** `FROM` lines of a Dockerfile that name no digest (a stage defined earlier, and scratch, are fine). */
export function unpinnedFroms(dockerfile, label = "the Dockerfile") {
  const problems = [];
  const earlierStages = new Set();
  for (const match of dockerfile.matchAll(/^FROM\s+(?:--platform=\S+\s+)?(\S+)(?:\s+AS\s+(\S+))?/gim)) {
    const [, image, stage] = match;
    if (image !== "scratch" && !earlierStages.has(image) && !DIGEST.test(image)) problems.push(`${label}'s FROM ${image} names no digest (use image:tag@sha256:...)`);
    if (stage) earlierStages.add(stage);
  }
  return problems;
}

/** The images the project pins somewhere: name before `:tag@sha256:...`, leaving out the project's own and variables. */
export function pinnedImageNames(files) {
  const names = new Set();
  for (const [, text] of files) {
    for (const match of text.matchAll(/(?<![\w./-])([a-z0-9][\w.-]*(?:\/[\w.-]+)*):[\w.-]+@sha256:[0-9a-f]{64}/g)) names.add(match[1]);
  }
  return names;
}

/**
 * Once an image is pinned anywhere, naming it again with a tag and no digest is a problem anywhere: in a workflow, a compose
 * file, a script or a Dockerfile. `files` is [[label, text]].
 */
export function unpinnedImages(files) {
  const problems = [];
  for (const name of pinnedImageNames(files)) {
    const moving = new RegExp(`(?<![\\w./-])${escapeRegExp(name)}:[\\w.-]+(?![\\w.-])(?!@sha256:[0-9a-f]{64})`, "g");
    for (const [label, text] of files) {
      for (const match of text.matchAll(moving)) problems.push(`${label} names ${match[0]}, which moves; pin it by digest like the other uses of ${name}`);
    }
  }
  return problems;
}

/** Every pin rule over the project's files: workflows, Dockerfile, compose files and the scripts that run images. */
export function pinProblems({ workflows, dockerfile, others }) {
  const files = [...workflows.map(([name, text]) => [`.github/workflows/${name}`, text]), ["Dockerfile", dockerfile], ...others];
  return [...unpinnedActions(workflows), ...unpinnedFroms(dockerfile), ...unpinnedImages(files)];
}
