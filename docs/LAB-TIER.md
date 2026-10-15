# Dioptra Lab

Dioptra stays free and MIT-licensed: every table, formula, analysis, graph and export, and
reading Prism files. **Dioptra Lab** is a paid license for the work that labs, core
facilities and regulated groups do on top of that: running the same analysis on many files,
reading instruments, handing figures to journals and data to Prism users, and keeping
records an auditor will accept.

## Evidence

**Labs are losing Prism.** Siemens bought Dotmatics, GraphPad's owner, in 2025 [1].
GraphPad no longer sells network or site licenses [3]. Universities pass the cost on: Weill
Cornell now charges $150 a year per machine "due to a vendor license cost increase" [4], and
Duke's per-computer charge rose from $60 to $71.50 [5]. On r/labrats, people write "my
institution isn't renewing our prism license" and "Our lab just lost its GraphPad license" [6][7].
A lab manager wrote that tracking licenses "is the bane of my existence" [6].

**Old files hold people back.** About a competitor that opens Prism files: "this will be
absolutely helpful for labs that are hesitant to stop paying for a license and being left with
a bunch of useless [files]" [6]. A postdoc who had lost Prism could not edit the figures a
reviewer asked for [8]. Other people need files *in* Prism format, because "My PI needs graphs
made using Prism" [9], and the R `pzfx` package has users who write `.pzfx` files for their
teams [10].

**Prism's own tiers lock basics away.** Since Prism 11, in-table formulas, multifactor ANOVA and
effect sizes beyond R² need the Pro plan [11]. A free tool that includes them beats Prism for a
student.

**Batch work is painful.** Prism's scripting can only import and export; the analysis has to
live in a template [12]. Users fit curves to "a bunch of CSV files" by duplicating families in
a loop [13], and people wish for "a cleaner way to visualize plate data without having to
wrangle it into Excel, GraphPad, or Python scripts every single time" [14].

**Figures still go through Illustrator.** The common advice is to set the exact size and
never resize. Otherwise people export each panel and "mess about … font sizes/line weights"
in Illustrator [15]. Users also complain about line widths changing when a layout is resized
in Prism 10.6 [16].

**Regulated labs have nothing affordable.** GraphPad says "Prism does not maintain an audit
trail" and that it "simply cannot comply" with 21 CFR Part 11 at its price [17]. Plate-reader
vendors sell GxP editions: Gen5 Secure lists at about $5,999 [18], BMG reader software at
$1,700–2,055 [19], and SoftMax Pro GxP needs at least three licenses, with validation sold as
a separate service [20]. Desktop software without a server is an accepted "closed system"
under 11.3(b)(4) when the vendor supplies the technical controls and the customer owns the
procedures [21].

**People pay to save time.** One r/labrats user called a Prism subscription "a $1 trade for
every hour you'd otherwise spend cursing ggplot" [22]. JASP is free and funds itself
through institutional memberships that include priority support [23].

## What Lab contains

Five features, in the order they will be built.

### 1. Batch analysis and instrument import

- **Templates.** Save any project as a template and apply it to a folder of new data files.
  Every analysis, graph and layout is recomputed. The output is one project per file plus a
  summary table, such as fitted EC50 values per plate, and figures rendered at their set size.
- **Plate readers.** Read the exports of Molecular Devices SoftMax Pro (`.txt`), BMG MARS
  (CSV), Agilent BioTek Gen5 (`.txt`) and Tecan Magellan (`.xlsx`), for both endpoint and
  kinetic reads, into XY or grouped tables, using a plate map of wells to samples and doses.
  The parsers are tested against the MIT-licensed sample files of Benchling's `allotropy` [24].
- **Watch folder.** Point a template at a folder; each new instrument export is analysed when
  it lands.
- `dioptra batch` runs the same work from the command line.

*Stays free:* importing a single plate-reader file, the full CLI, and re-running a project on
new data by hand. These are what Prism's Wand and families give every user.

### 2. Journal figures and reports

- **Journal presets.** Width (single column, 1.5 column, double), font and minimum size, line
  widths, DPI, and file type (TIFF with LZW, EPS, PDF) for Nature, Cell Press, Science, PLOS,
  eLife, the ACS and Elsevier families, and the APS and JPhysiol journals. A preset can be
  applied to every graph and layout in a project at once.
- **Figure check.** Before export, check text below the journal minimum, lines thinner than
  0.25 pt, raster resolution, colour pairs a colour-blind reader cannot tell apart, and panels
  whose fonts differ.
- **Layouts at final size.** Panel letters, and the same font and line widths in every panel,
  whatever the panel size, so figures no longer need finishing in Illustrator.
- **Methods and report.** A methods paragraph naming each test, its options, the correction
  for multiple comparisons, the software version and the references to cite. A Word (`.docx`)
  or PDF report of the data, results sheets and figures for a project.

*Stays free:* SVG, PDF and PNG export at any size, multi-panel layouts, colour-blind-safe
palettes, and the one-line methods text on each results sheet.

### 3. Prism hand-off

- **Write `.pzfx`.** Data tables, with replicates, formats, row titles and exclusions, go to a
  file that Prism 5 and later opens, for a PI or a collaborator who still uses Prism. Prism
  encrypts the analysis and graph parts of its own files [25], so those are not written.
- **Batch conversion.** Convert a folder of `.pzfx` and `.prism` files to `.dioptra` in one
  step, with a report of anything that did not carry over.

*Stays free:* opening `.pzfx` and `.prism` files, however many. Reading old files is what
lets a lab switch, and it must never be held back.

### 4. Records for regulated labs

These are the technical controls of 21 CFR Part 11 and EU Annex 11, for a desktop computer
without a server:

- **Users and roles.** Named users with passwords and roles (analyst, reviewer, admin),
  kept per computer, in the style of SoftMax Pro GxP's local user database [20].
- **Audit trail.** Every change to data, formulas, exclusions, analysis options and graphs is
  recorded with who, when, what and why. The trail is append-only, hash-chained into the
  project file, and cannot be turned off for a regulated project.
- **Electronic signatures.** Name, time and meaning (authored, reviewed, approved), with the
  password asked again, and bound to the record's hash. A signed record is locked; changing
  it starts a new version that must be signed again.
- **Validation pack.** A Part 11 and Annex 11 compliance matrix, a requirements traceability
  matrix, IQ and OQ protocols, and an automated OQ that reruns the NIST StRD and oracle suites
  ([correctness](CORRECTNESS.md)) on the installed build and writes a signed report. The
  customer writes the SOPs and runs the PQ, as with every vendor.

Dioptra describes this as *supporting* Part 11 compliance, never as "compliant", as other
vendors do [21].

*Stays free:* the command history every project already keeps, and checking a file's audit
trail and signatures. Anyone can verify a signed record without a license.

### 5. Site licenses and support

- **Site license.** One key covers a department or institution. There is no license server,
  no seat counting and nothing to install beyond the app. GraphPad no longer offers this [3].
- **Shared templates.** A template folder on a network drive or in a synced folder, shown as
  a library in every copy of the app at the site.
- **Priority support.** Email support with a reply within two working days, numerical
  questions answered with a reference, and early builds.

## Cut

| Candidate | Why it is not in Lab |
|---|---|
| Prism import (`.pzfx`, `.prism`) | It brings users to Dioptra; holding it back would keep labs on Prism |
| CLI and scripting | Reproducibility is a core promise and the CLI is MIT already; Lab adds `batch` only |
| Effect sizes, formulas, multifactor ANOVA | Prism puts these behind Pro [11]; including them free is why free beats Prism |
| Cloud sync, real-time collaboration | Needs a service, accounts and storage; the file diffs under git |
| Site-license admin console | A site key has no seats to manage; an admin console is a server |

## Prices

| License | Price | For |
|---|---|---|
| Academic | $79 a year | One person at a university, hospital or non-profit |
| Corporate | $199 a year | One person at a company |
| Founding member | $49 a year | The first 200 buyers, academic or corporate; the price holds while renewed |
| Department site | $990 a year | Up to 50 people at one academic department |
| Institution site | $2,900 a year | Everyone at one academic institution |
| Corporate site | Quoted | A company site, with the validation pack and OQ support |

For comparison, Prism costs $260 academic and $520 corporate per person per year, and $2,600
for ten academic seats [2]. Students use the free core: everything a student needs is in it.

## The license

- **Offline.** A key is a signed statement (ed25519) of who it is for, the plan, the seats
  and the dates. The app checks the signature against a public key compiled into it. It never
  contacts a server, so it works on air-gapped instruments and behind firewalls.
- **Per person.** A person may use their key on every computer where they are the main user.
  A site key covers everyone at the site.
- **When it ends.** Lab features stay on for 30 days after the end date and then pause.
  Projects, data and everything made with Lab keep opening, reports and figures stay as they
  are, audit trails and signatures stay verifiable, and templates can still be applied one
  file at a time. A renewed key turns Lab back on.
- **Source.** The Lab code is published in this repository under the Dioptra Lab License: it
  may be read and audited, as a regulated buyer needs, but using it requires a key. Everything
  else is MIT.

## Build order and weeks

| Weeks | Work |
|---|---|
| 1 | License verifier, activation in Settings, lapse behaviour, `dioptra license` |
| 1–2 | Prism import tested on 25 real `.pzfx` files, `.prism` import, fixes |
| 2–3 | Plate-reader import (free, single file), batch templates and watch folder (Lab) |
| 3–4 | Journal presets, figure check, methods and report export |
| 4–5 | `.pzfx` writing, batch conversion |
| 5–7 | Users, audit trail, signatures, validation pack and automated OQ |
| 2–8 | Site: comparisons, a page per analysis, pricing, checkout (Paddle) |

Batch and figures come first because academic buyers make up the early revenue and the
founding-member launch. The records pack is last because it is the largest piece and it sells
through site quotes, not checkout.

## Sources

1. [Siemens completes acquisition of Dotmatics](https://www.automation.com/article/siemens-completes-acquisition-dotmatics)
2. [GraphPad: How to buy](https://www.graphpad.com/how-to-buy/)
3. [GraphPad: Are network or site licenses available?](https://www.graphpad.com/support/faq/are-network-or-site-licenses-available-for-graphpad-software)
4. [Weill Cornell Medicine Library: GraphPad Prism](https://library.weill.cornell.edu/graphpad-prism)
5. [Duke OIT: GraphPad Prism renewed](https://sites.duke.edu/software/2026/01/08/graphpad-prism-renewed-5)
6. [r/labrats: Feedback on new graph/stat software](https://reddit.com/r/labrats/comments/1wvbt4w/)
7. [r/labrats: Our lab just lost its GraphPad license](https://reddit.com/r/labrats/comments/1uyjvfr/)
8. [r/labrats: How to get GraphPad Prism for free](https://reddit.com/r/labrats/comments/1bp2lii/)
9. [r/bioinformatics: Prism automation of data input](https://reddit.com/r/bioinformatics/comments/xwu1wb/)
10. [pzfx R package, issue 18](https://github.com/Yue-Jiang/pzfx/issues/18)
11. [GraphPad: Restricted features](https://www.graphpad.com/support/faq/restricted-features/)
12. [Prism guide: Alternatives to scripting](https://graphpad.com/guides/prism/latest/user-guide/althernatives_to_scripting.htm)
13. [r/labrats: Fitting many CSV files in Prism](https://reddit.com/r/labrats/comments/ec0cwe/)
14. [r/labrats: Visualising plate data](https://reddit.com/r/labrats/comments/1ujbfiq/)
15. [r/labrats: Consistent text sizes across figures](https://reddit.com/r/labrats/comments/1pd1i3k/)
16. [r/labrats: Line thickness in Prism 10.6 layouts](https://reddit.com/r/labrats/comments/1tkxm1g/)
17. [GraphPad: Has Prism been validated? 21 CFR Part 11](https://www.graphpad.com/support/faq/has-prismandnbspbeen-validated-does-it-comply-with-the-fda-regulation-21-cfr-part-11/)
18. [Fisher Scientific: Gen5 software](https://www.fishersci.com/shop/products/gen5-data-collection-analysis-software/11120510)
19. [BMG Labtech store: Omega software](https://store.bmglabtech.com/product/bmg-labtech-software-omega/)
20. [Molecular Devices: SoftMax Pro GxP](https://www.moleculardevices.com/products/gxp-compliance-solutions/softmax-pro-gxp-software); [GxP Admin guide](https://moleculardevices.com/sites/default/files/en/assets/user-guide/br/gxpadmin-userguide-v7-2-5305575a.pdf)
21. [Agilent: ProteoAnalyzer and Part 11](https://www.agilent.com/cs/library/whitepaper/public/wp-title-21-cfr-part-11-annex-11-proteoanalyzer-agilent-5994-8836en.pdf); [21 CFR 11.10](https://www.law.cornell.edu/cfr/text/21/11.10)
22. [r/labrats: Is GraphPad Prism worth it?](https://reddit.com/r/labrats/comments/12zwm6a/is_graphpad_prism_worth_it_any_better_and_cheaper/jhuesi4/)
23. [JASP Cooperative](https://jasp-stats.org/?p=9872)
24. [Benchling allotropy](https://github.com/Benchling-Open-Source/allotropy)
25. [Prism guide: .pzf and .pzfx formats](https://www.graphpad.com/guides/prism/latest/user-guide/pzf_vs__pzfx_file_format.htm)
