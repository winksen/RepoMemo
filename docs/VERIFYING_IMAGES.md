# Verifying a release image

Every image on the registry (`ghcr.io/<owner>/repomemo-server`, `-converter`, `-edge` and `-web`) is built by the repository's own
`release-images` workflow on a version tag, and carries two statements signed by GitHub's workflow identity with Sigstore (no key to
keep, no key to leak):

- **a build attestation** (SLSA provenance): which repository, which workflow, which commit and which tag built that exact image;
- **an SBOM** (SPDX): every package inside it, read from the pushed image.

Both are tied to the image's digest, so a different image does not verify. Verifying is optional; nothing in RepoMemo needs it to run.

## Verify the build

You need the [GitHub CLI](https://cli.github.com) (`gh`), signed in, and Docker only if you also pull the image.

```bash
gh attestation verify oci://ghcr.io/<owner>/repomemo-server:X.Y.Z --repo <owner>/repomemo
```

It prints the repository, the workflow file and the commit when the attestation is valid, and fails when the image was not built by
that repository's workflow. Do the same for `repomemo-converter`, `repomemo-edge` and `repomemo-web`.

## Read the SBOM

```bash
gh attestation verify oci://ghcr.io/<owner>/repomemo-server:X.Y.Z --repo <owner>/repomemo \
  --predicate-type https://spdx.dev/Spdx --format json
```

The packages are in the `predicate` of the result. To see the signatures themselves with [cosign](https://docs.sigstore.dev):

```bash
cosign verify-attestation --type slsaprovenance1 \
  --certificate-identity-regexp '^https://github.com/<owner>/repomemo/\.github/workflows/release-images\.yml@refs/tags/v' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  ghcr.io/<owner>/repomemo-server:X.Y.Z
```

## What it does and does not tell you

- It tells you the image came from this repository's workflow, from a tag, at a named commit, and what it holds.
- It does not tell you the code is free of faults: the images are scanned for known vulnerabilities in CI (`image-scan`), which is a
  separate statement.
- Pin by digest when you deploy: `docker pull ghcr.io/<owner>/repomemo-server@sha256:...` is the image that was verified, whereas a tag
  such as `latest` can move to a newer release.
