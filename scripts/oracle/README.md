# Oracle scripts

Regenerate the fixtures from the project root. `ORACLE_PY` must point at a Python
with numpy, scipy, statsmodels, lifelines, scikit-learn, pandas and mpmath:

```sh
"$ORACLE_PY" -I scripts/oracle/parse_nist.py     # fixtures/nist/*.dat -> *.json
"$ORACLE_PY" -I scripts/oracle/gen_fixtures.py   # fixtures/oracle/*.json (about 2 minutes)
```

The output is deterministic: running it twice gives byte-identical files.
See `research/05-nist-and-oracle-notes.md` for the layout and tolerance policy.
