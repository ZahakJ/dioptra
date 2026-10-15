# Metrics

Dioptra contains no telemetry, analytics or tracking. The app never contacts a server,
including for license checks. Every number below comes from outside the app.

## Week-12 kill check

Stop and move the time to Loci if **any** of these holds on the week-12 review:

| Measure | Threshold | Source |
|---|---|---|
| Downloads, all releases, all platforms | < 3,000 | `scripts/downloads.sh` → `downloads.csv`, last line, `total` |
| Paying Lab seats | < 30 | Paddle dashboard |
| Site-license pilots started | 0 | `cores.csv`, rows with a `pilot` date |

Above all three: start the corporate tier and the validation pack.

Write the result in one line at the bottom of this file:
`2027-01-10: downloads N, seats N, pilots N → continue / stop`.

## Where each number comes from

**Downloads.** GitHub counts each asset download. Run from the repository root:

```sh
scripts/downloads.sh                 # appends to docs/launch/downloads.csv
scripts/downloads.sh /tmp/x.csv      # any other file
```

It prints a table per release and platform (AppImage, deb, rpm, msi, exe, dmg) and
appends `date,total,appimage,deb,rpm,msi,exe,dmg,other`. It uses `gh api` when `gh` is
logged in, otherwise anonymous `curl` (60 requests an hour). Needs `jq`. GitHub's count
includes reinstalls and bots; treat it as an upper bound.

**Paying seats.** Paddle → Customers → Subscriptions, status *active*. Count seats, not
subscriptions: a site license counts as the `seats` in its key. Exclude `trial` keys and
refunds.

**Pilots.** `docs/launch/cores.csv`: count rows with a `pilot` date. A pilot counts when
its key is issued and installed, not when someone says yes.

**Secondary, for the weekly line only:** GitHub stars and open issues
(`gh repo view ZahakJ/dioptra --json stargazerCount`), new issues filed by people
other than you, user calls done, and Insights → Traffic (views and clones, 14 days only).

## Weekly routine (10 min, Sunday)

1. `scripts/downloads.sh` (1 min). Note the week's increase.
2. Paddle dashboard: active seats, new this week, churn (2 min). From week 8.
3. `cores.csv`: emails sent, replies, pilots (2 min). From week 8.
4. GitHub: stars, new issues, issues closed (2 min).
5. Add one line below (3 min):

```text
week  date        downloads(+wk)  seats  pilots  stars  calls  note
1     2026-10-25  11 (+0)         0      0       0      0      baseline
```

Six weeks of flat downloads after the launch is the early warning; do not wait for week 12
to read it.

## Log

```text
week  date        downloads(+wk)  seats  pilots  stars  calls  note
0     2026-10-14  11              0      0       0      0      baseline before launch
```
