# Change control

How Dioptra versions are numbered and released, where changes are documented, and which
changes call for re-validation.

## Versions

Dioptra follows [semantic versioning](https://semver.org/): `MAJOR.MINOR.PATCH`.

| Part | Raised for |
|---|---|
| PATCH | Defect fixes; no change to documented behaviour other than the fix |
| MINOR | New functions or options; existing results and files unchanged unless a fix says otherwise |
| MAJOR | Incompatible changes to results, files or commands |

The project file format has its own version (`dioptra/1.x`,
[FILE-FORMAT.md](../FILE-FORMAT.md)). A reader opens every file of its own major version or
older; older files are migrated when opened, and saving writes the current format.

The command-line tool, the desktop app, the web app and Dioptra Lab share one version
number, and every installer of a version is built from the same tagged commit.

## Release process

1. Every change is a commit in the public repository `github.com/ZahakJ/dioptra`.
2. The release gate (`./verify`) must pass: formatting, static analysis (clippy, warnings
   as errors), the full test suite (including the NIST StRD and oracle comparisons and the
   records tests), the web engine build, front-end type check and unit tests, the desktop
   and command-line builds and the end-to-end browser tests. Continuous integration runs
   it on every push.
3. A release is a tag `vX.Y.Z`. The release workflow (`.github/workflows/release.yml`)
   builds the installers for every platform from that tag on GitHub-hosted runners,
   verifies the macOS signature, uploads the installers to the GitHub release and
   publishes `SHA256SUMS.txt` for every file of the release.
4. Published releases are not replaced. A correction is a new version.

## Release notes

- `CHANGELOG.md` in the repository, one section per version with its date, grouped by area;
  also the Changelog chapter of the manual.
- The GitHub release page of each version, with its installers and `SHA256SUMS.txt`.
- Changes to numerical results are stated as such in the changelog, with the method and
  the direction of the change. Changes to the records controls are listed under their own
  heading.

## Assessing an update

The customer assesses each new version before installing it on validated systems:

| Change in the release notes | Typical action |
|---|---|
| Any new version | IQ on the updated computers; automated OQ (OQ-A). |
| Records controls (users, audit trail, signatures, verification) | Repeat MOQ-1 to MOQ-4 or the affected scripts. |
| Numerical change to an analysis in the intended use | Repeat the PQ tests of that analysis. |
| New analysis or option not in the intended use | No PQ; keep it out of use or extend the intended use and PQ. |
| File format version change | Open and verify representative regulated projects; check that their trails verify intact. |
| Operating-system upgrade or new hardware, same Dioptra version | Automated OQ (OQ-A). |

The automated OQ in a new version covers every comparison of that version's test suite;
its fixture-set SHA-256 changes when reference data are added or corrected, and the
changelog says why.

## Customer configuration

Dioptra has no server-side configuration. The items under customer change control are the
installed version, the users file (accounts and roles; every change is in its access log),
the license key and the storage locations of regulated projects.
