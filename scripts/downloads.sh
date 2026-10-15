#!/usr/bin/env bash
# Sum GitHub release download counts per release and per platform, append one dated
# line to a CSV, and print a table. Uses `gh api` when gh is logged in, else curl.
#
#   scripts/downloads.sh [file.csv]      default: docs/launch/downloads.csv
#
# CSV columns: date,total,appimage,deb,rpm,msi,exe,dmg,other
set -euo pipefail

repo=ZahakJ/dioptra
csv=${1:-docs/launch/downloads.csv}
cols=(appimage deb rpm msi exe dmg other)

fetch() {
	if command -v gh >/dev/null && gh auth status >/dev/null 2>&1; then
		gh api --paginate "repos/$repo/releases?per_page=100"
	else
		curl -fsSL -H 'Accept: application/vnd.github+json' \
			"https://api.github.com/repos/$repo/releases?per_page=100"
	fi
}

command -v jq >/dev/null || { echo "downloads.sh: needs jq" >&2; exit 1; }

# One line per asset: tag<TAB>platform<TAB>count. gh --paginate prints one array per page.
rows=$(fetch | jq -r '
	.[] | .tag_name as $t | .assets[] |
	(.name | ascii_downcase) as $n |
	[ $t,
	  ( if   ($n | endswith(".appimage")) then "appimage"
	    elif ($n | endswith(".deb"))      then "deb"
	    elif ($n | endswith(".rpm"))      then "rpm"
	    elif ($n | endswith(".msi"))      then "msi"
	    elif ($n | endswith(".exe"))      then "exe"
	    elif ($n | endswith(".dmg"))      then "dmg"
	    else "other" end ),
	  (.download_count | tostring) ] | @tsv')

if [ -z "$rows" ]; then
	echo "downloads.sh: no release assets found for $repo" >&2
	exit 1
fi

# Table: one row per release, one column per platform, then a total row.
printf '%s\n' "$rows" | awk -F'\t' -v cols="${cols[*]}" '
	BEGIN { n = split(cols, c, " ") }
	{
		if (!($1 in seen)) { seen[$1] = 1; order[++r] = $1 }
		cnt[$1, $2] += $3; rel[$1] += $3; plat[$2] += $3; all += $3
	}
	END {
		printf "%-10s", "release"
		for (i = 1; i <= n; i++) printf " %9s", c[i]
		printf " %9s\n", "total"
		for (j = 1; j <= r; j++) {
			t = order[j]; printf "%-10s", t
			for (i = 1; i <= n; i++) printf " %9d", cnt[t, c[i]]
			printf " %9d\n", rel[t]
		}
		printf "%-10s", "all"
		for (i = 1; i <= n; i++) printf " %9d", plat[c[i]]
		printf " %9d\n", all
	}'

line=$(printf '%s\n' "$rows" | awk -F'\t' -v cols="${cols[*]}" -v d="$(date +%F)" '
	BEGIN { n = split(cols, c, " ") }
	{ plat[$2] += $3; all += $3 }
	END {
		printf "%s,%d", d, all
		for (i = 1; i <= n; i++) printf ",%d", plat[c[i]]
		printf "\n"
	}')

mkdir -p "$(dirname "$csv")"
if [ ! -s "$csv" ]; then
	echo "date,total,$(IFS=,; echo "${cols[*]}")" >"$csv"
fi
echo "$line" >>"$csv"
echo
echo "appended to $csv: $line"
