# Core facilities

Weeks 8–9: 50 emails to core-facility managers, 25 a week. Goal: at least one free
60-day site-license pilot. A pilot key is a `trial` key with `seats` and `site` set (see
`~/Documents/side/dioptra-keys/README.md`).

## Who

Managers or directors of cores that hand data to many labs: plate-reader and screening
cores, flow cytometry, imaging, proteomics, genomics QC, and biostatistics consulting
cores. Prefer cores that list Prism as a supported program, train users, or own a plate
reader.

## Email

Plain text, from your own address, to one person, never in bulk. Personalise the first
line from their facility page.

**Subject lines** (pick one, rotate):

- Free site pilot: stats and figures software for [facility]
- A free Prism alternative for [facility]'s users
- [Facility] users and Prism licences
- Question about how [facility] users analyse plate-reader data

**Body (≤150 words):**

> Dear Dr [Name],
>
> I saw that [facility] [runs a SpectraMax / trains users on Prism / supports N labs].
> I build Dioptra, a free open-source desktop program for lab statistics and figures:
> t tests to nonlinear dose-response fits, journal-sized graphs, and it opens Prism
> `.pzfx` files. It runs on Windows, macOS and Linux, offline, with no account.
>
> I'd like to offer [facility] a free 60-day site pilot of Dioptra Lab, the paid add-on:
> batch analysis of plate-reader exports with templates, a watch folder, journal figure
> presets and report export. One key covers every computer in the facility; no licence
> server.
>
> In return I'd ask for 20 minutes of feedback at the end.
>
> Would that be useful? The manual is at https://zahakj.github.io/dioptra/manual/.
>
> Best regards,
> [Name], [role, affiliation if any]

**Follow-up after 5 working days** (reply in the same thread):

> Dear Dr [Name], a short follow-up in case this got buried. If a pilot isn't a fit, would
> someone else at [institution] be the right person to ask? Thank you.

One follow-up only. No reply after that: mark it and move on.

**On a yes:** issue the key the same day, send it with the download link and a one-page
"what to try first" (open the sample project, run a template on one plate folder). Book
the 20-minute call for day 50 now. Note the date in the CSV.

## How to find 50 managers

Collect 60–70 rows in one or two sittings in week 7, before sending anything.

- **ABRF Core Administrators directory and ABRF member cores**: https://www.abrf.org
  (Core Directory). Filter by technology.
- **CoreMarketplace**: https://coremarketplace.org. Search "plate reader", "flow
  cytometry", "high-throughput screening"; each listing names the director.
- **iLab Solutions (Agilent CrossLab) core listings**: many universities publish their
  iLab core list; search `site:ilab.agilent.com "core"` or the institution's iLab page.
- **University "core facilities" pages**, by search engine:
  - `site:.edu "core facility" "plate reader"`
  - `site:.edu "core facility" ("flow cytometry" OR "high-throughput screening") "GraphPad"`
  - `site:.edu "shared resource" "plate reader" director`
  - `site:.ac.uk "core facility" "plate reader"` and the same for `.edu.au`, `.ca`, `.de`
- **NIH-funded shared instruments**: NIH RePORTER (https://reporter.nih.gov), activity
  codes S10 (Shared Instrumentation) and P30 (centre core grants); the PI is often the core
  director. Search "plate reader" or "flow cytometer" in S10 awards.
- **NCI-designated cancer centre shared resources**: each centre lists its cores and
  managers.
- **EU networks**: Core Technologies for Life Sciences (CTLS, https://ctls-org.eu) member
  list; Euro-BioImaging nodes (https://www.eurobioimaging.eu); EU-OPENSCREEN partner sites
  for screening cores.
- **University software catalogues**: the IT pages that sell Prism licences (as Duke and
  Weill Cornell do, LAB-TIER.md sources 4–5). The software licensing contact is a second
  buyer; add them with facility "IT software".

Skip a row if the page has no named person; a generic `core@` inbox rarely starts a pilot.

## CSV

Keep it at `docs/launch/cores.csv`, out of git if it holds personal emails (add it to
`.git/info/exclude`).

```csv
name,institution,facility,email,source,date_contacted,follow_up,reply,pilot,notes
Dr A. Example,Example University,Flow Cytometry Core,a.example@example.edu,CoreMarketplace,2026-12-07,2026-12-14,yes,2026-12-08,SpectraMax; 40 labs
```

`reply`: blank, `no`, `yes`, `referred`. `pilot`: blank or the key's issue date.
