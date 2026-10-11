# Security

Dioptra is a desktop application that reads and writes files you choose. It makes no
network requests except the optional check of the GitHub releases page from the landing
site (not from the app).

## Reporting

Please report vulnerabilities privately through GitHub's "Report a vulnerability" on this
repository (Security tab) rather than in a public issue. Include the version, platform and
a reproduction. You will get an acknowledgement within a week.

## Scope

- Project files (`.dioptra`) are zip archives of JSON; the reader rejects paths outside the
  archive and caps sizes.
- Imported files (CSV, XLSX, PZFX) are parsed with pure-Rust parsers in the app process.
- The formula language has no file or network access.

Supported versions: the latest release.
