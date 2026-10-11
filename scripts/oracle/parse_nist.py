"""Parse NIST StRD .dat files in fixtures/nist/ into fixtures/nist/<name>.json.

Run from the project root:  python -I scripts/oracle/parse_nist.py

Each JSON keeps every certified value twice: as a JSON float and as the exact
text printed by NIST (``*_text``), so no digit is lost. For nonlinear sets the
model is also given as an expression string in a small grammar that the Rust
test harness can evaluate:

    numbers, x (or x1, x2), b1..b9, pi, + - * / ^ (power, right assoc),
    unary minus, functions exp log sin cos atan sqrt.

The parser verifies every nonlinear expression by evaluating the residual sum
of squares at the certified parameters and comparing with the certified RSS.
"""

import json
import math
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
NIST = os.path.join(ROOT, "fixtures", "nist")
BASE = "https://www.itl.nist.gov/div898/strd"

NLS = ["Misra1a", "Chwirut2", "Chwirut1", "Lanczos3", "Gauss1", "Gauss2", "DanWood",
       "Misra1b", "Kirby2", "Hahn1", "Nelson", "MGH17", "Lanczos1", "Lanczos2", "Gauss3",
       "Misra1c", "Misra1d", "Roszman1", "ENSO", "MGH09", "Thurber", "BoxBOD", "Rat42",
       "MGH10", "Eckerle4", "Rat43", "Bennett5"]
LLS = ["Norris", "Pontius", "NoInt1", "NoInt2", "Filip", "Longley",
       "Wampler1", "Wampler2", "Wampler3", "Wampler4", "Wampler5"]
ANOVA = ["SiRstv"] + ["SmLs0%d" % i for i in range(1, 10)] + ["AtmWtAg"]
UNIV = ["PiDigits", "Lottery", "Lew", "Mavro", "Michelso",
        "NumAcc1", "NumAcc2", "NumAcc3", "NumAcc4"]

LANCZOS = "b1*exp(-b2*x) + b3*exp(-b4*x) + b5*exp(-b6*x)"
GAUSS = "b1*exp(-b2*x) + b3*exp(-(x-b4)^2/b5^2) + b6*exp(-(x-b7)^2/b8^2)"
CUBIC_CUBIC = "(b1 + b2*x + b3*x^2 + b4*x^3)/(1 + b5*x + b6*x^2 + b7*x^3)"
EQUATIONS = {
    "Misra1a": "b1*(1-exp(-b2*x))",
    "Chwirut2": "exp(-b1*x)/(b2+b3*x)",
    "Chwirut1": "exp(-b1*x)/(b2+b3*x)",
    "Lanczos3": LANCZOS, "Lanczos1": LANCZOS, "Lanczos2": LANCZOS,
    "Gauss1": GAUSS, "Gauss2": GAUSS, "Gauss3": GAUSS,
    "DanWood": "b1*x^b2",
    "Misra1b": "b1*(1-(1+b2*x/2)^(-2))",
    "Kirby2": "(b1 + b2*x + b3*x^2)/(1 + b4*x + b5*x^2)",
    "Hahn1": CUBIC_CUBIC, "Thurber": CUBIC_CUBIC,
    # Nelson: the RESPONSE is log(y); fit f to log(y) (two predictors x1, x2).
    "Nelson": "b1 - b2*x1*exp(-b3*x2)",
    "MGH17": "b1 + b2*exp(-x*b4) + b3*exp(-x*b5)",
    "Misra1c": "b1*(1-(1+2*b2*x)^(-0.5))",
    "Misra1d": "b1*b2*x*((1+b2*x)^(-1))",
    "Roszman1": "b1 - b2*x - atan(b3/(x-b4))/pi",
    "ENSO": ("b1 + b2*cos(2*pi*x/12) + b3*sin(2*pi*x/12) + b5*cos(2*pi*x/b4) "
             "+ b6*sin(2*pi*x/b4) + b8*cos(2*pi*x/b7) + b9*sin(2*pi*x/b7)"),
    "MGH09": "b1*(x^2+x*b2)/(x^2+x*b3+b4)",
    "BoxBOD": "b1*(1-exp(-b2*x))",
    "Rat42": "b1/(1+exp(b2-b3*x))",
    "MGH10": "b1*exp(b2/(x+b3))",
    "Eckerle4": "(b1/b2)*exp(-0.5*((x-b3)/b2)^2)",
    "Rat43": "b1/((1+exp(b2-b3*x))^(1/b4))",
    "Bennett5": "b1*(b2+x)^(-1/b3)",
}

LLS_MODEL = {
    "Norris": {"kind": "polynomial", "degree": 1, "intercept": True},
    "Pontius": {"kind": "polynomial", "degree": 2, "intercept": True},
    "NoInt1": {"kind": "polynomial", "degree": 1, "intercept": False},
    "NoInt2": {"kind": "polynomial", "degree": 1, "intercept": False},
    "Filip": {"kind": "polynomial", "degree": 10, "intercept": True},
    "Longley": {"kind": "multiple", "predictors": 6, "intercept": True},
}
for _w in range(1, 6):
    LLS_MODEL["Wampler%d" % _w] = {"kind": "polynomial", "degree": 5, "intercept": True}

NUM = r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[EeDd][-+]?\d+)?"


def fnum(s):
    s = s.strip()
    if s.lower() in ("infinity", "inf"):
        return None
    return float(s.replace("D", "E").replace("d", "e"))


def read(name):
    with open(os.path.join(NIST, name + ".dat"), encoding="latin-1") as fh:
        return fh.read().splitlines()


def ranges(lines):
    head = "\n".join(lines[:15])
    out = {}
    for key, pat in (("start", r"Starting Values\s*\(lines\s*(\d+)\s*to\s*(\d+)"),
                     ("cert", r"Certified Values\s*:?\s*\(?lines\s*(\d+)\s*to\s*(\d+)"),
                     ("data", r"Data\s*:?\s*\(?lines\s*(\d+)\s*to\s*(\d+)")):
        m = re.search(pat, head, re.I)
        if m:
            out[key] = (int(m.group(1)), int(m.group(2)))
    return out


# Michelso.dat omits the difficulty line; the NIST univariate index page lists it as Lower.
DIFFICULTY_OVERRIDE = {"Michelso": "Lower"}


def difficulty(lines, name=None):
    if name in DIFFICULTY_OVERRIDE:
        return DIFFICULTY_OVERRIDE[name]
    m = re.search(r"(Lower|Average|Higher)\s+Level\s+of\s+Difficulty", "\n".join(lines), re.I)
    return m.group(1).capitalize() if m else None


def data_rows(lines, rng, ncol):
    a, b = rng
    rows = []
    for ln in lines[a - 1:b]:
        toks = ln.split()
        if not toks:
            continue
        rows.append([fnum(t) for t in toks])
    assert all(len(r) == ncol for r in rows), "bad column count"
    assert len(rows) == b - a + 1, (len(rows), a, b)
    return rows


def header_cols(lines, rng):
    # the "Data:" header is the line just before the data block
    ln = lines[rng[0] - 2]
    assert ln.strip().startswith("Data"), ln
    cols = ln.split(":", 1)[1].split()
    return cols


def model_text(lines):
    """The raw text of the header's Model: section (whitespace collapsed)."""
    out, on = [], False
    for ln in lines[:60]:
        if re.match(r"^Model:", ln):
            on = True
            out.append(ln.split(":", 1)[1].strip())
            continue
        if on:
            if re.match(r"^\S", ln) or re.search(r"Starting [Vv]alues|Certified", ln):
                break
            if ln.strip():
                out.append(ln.strip())
    return " | ".join(x for x in out if x)


# ---------------------------------------------------------------- expression check
def py_expr(eq):
    s = eq.replace("^", "**")
    return s


def eval_model(eq, params, xcols):
    env = {"exp": math.exp, "log": math.log, "sin": math.sin, "cos": math.cos,
           "atan": math.atan, "sqrt": math.sqrt, "pi": math.pi}
    env.update(params)
    env.update(xcols)
    return eval(compile(py_expr(eq), "<eq>", "eval"), {"__builtins__": {}}, env)


def text_value(t):
    return {"value": fnum(t), "text": t}


def parse_nls(name):
    L = read(name)
    r = ranges(L)
    params = []
    for ln in L[r["cert"][0] - 1:r["cert"][1]]:
        m = re.match(r"\s*(b\d+)\s*=\s*(%s)\s+(%s)\s+(%s)\s+(%s)\s*$" % (NUM, NUM, NUM, NUM), ln)
        if m:
            params.append({"name": m.group(1), "start1": fnum(m.group(2)), "start2": fnum(m.group(3)),
                           "certified": fnum(m.group(4)), "certified_text": m.group(4),
                           "std_dev": fnum(m.group(5)), "std_dev_text": m.group(5)})
    txt = "\n".join(L[r["cert"][0] - 1:r["cert"][1] + 2])

    def grab(label):
        m = re.search(label + r"\s*:?\s*(%s)" % NUM, txt)
        return m.group(1)
    rss = grab("Residual Sum of Squares")
    rsd = grab("Residual Standard Deviation")
    dof = int(grab("Degrees of Freedom"))
    nobs = int(grab("Number of Observations"))
    cols = header_cols(L, r["data"])
    rows = data_rows(L, r["data"], len(cols))
    assert len(rows) == nobs
    data = {c: [row[i] for row in rows] for i, c in enumerate(cols)}
    eq = EQUATIONS[name]
    # verify equation against certified RSS
    pv = {p["name"]: p["certified"] for p in params}
    ss = 0.0
    for i in range(nobs):
        xs = {c: data[c][i] for c in cols if c != "y"}
        if "x" not in xs and "x1" in xs:
            pass
        yv = math.log(data["y"][i]) if name == "Nelson" else data["y"][i]
        ss += (yv - eval_model(eq, pv, xs)) ** 2
    rel = abs(ss - fnum(rss)) / fnum(rss)
    # Lanczos1 has RSS ~1e-25: 11-digit parameters cannot reproduce it, so
    # also accept an absolute agreement scaled by sum(y^2).
    sy2 = sum(v * v for v in data["y"])
    assert rel < 1e-6 or abs(ss - fnum(rss)) < 1e-15 * sy2, (name, ss, rss, rel)
    return {
        "name": name, "class": "nonlinear_regression", "difficulty": difficulty(L, name),
        "source_url": "%s/nls/data/LINKS/DATA/%s.dat" % (BASE, name),
        "model_text": model_text(L),
        "equation": eq,
        "response_transform": "log" if name == "Nelson" else None,
        "predictors": [c for c in cols if c != "y"],
        "n_params": len(params),
        "parameters": params,
        "residual_sum_of_squares": text_value(rss),
        "residual_std_dev": text_value(rsd),
        "degrees_of_freedom": dof,
        "n_observations": nobs,
        "check_rss_at_certified_params": ss,
        "data": data,
    }


def parse_lls(name):
    L = read(name)
    r = ranges(L)
    block = L[r["cert"][0] - 1:r["cert"][1]]
    params = []
    for ln in block:
        m = re.match(r"\s*(B\d+)\s+(%s)\s+(%s)\s*$" % (NUM, NUM), ln)
        if m:
            params.append({"name": m.group(1), "certified": fnum(m.group(2)), "certified_text": m.group(2),
                           "std_dev": fnum(m.group(3)), "std_dev_text": m.group(3)})
    txt = "\n".join(block)
    rsd = re.search(r"Standard Deviation\s+(%s)" % NUM, txt).group(1)
    r2 = re.search(r"R-Squared\s+(%s)" % NUM, txt).group(1)
    reg = re.search(r"Regression\s+(\d+)\s+(%s)\s+(%s)\s+(%s|Infinity)" % (NUM, NUM, NUM), txt)
    res = re.search(r"Residual\s+(\d+)\s+(%s)\s+(%s)" % (NUM, NUM), txt)
    cols = header_cols(L, r["data"])
    rows = data_rows(L, r["data"], len(cols))
    data = {c: [row[i] for row in rows] for i, c in enumerate(cols)}
    return {
        "name": name, "class": "linear_regression", "difficulty": difficulty(L, name),
        "source_url": "%s/lls/data/LINKS/DATA/%s.dat" % (BASE, name),
        "model_text": model_text(L),
        "model": LLS_MODEL[name],
        "parameters": params,
        "residual_std_dev": text_value(rsd),
        "r_squared": text_value(r2),
        "anova": {
            "regression": {"df": int(reg.group(1)), "ss": text_value(reg.group(2)),
                           "ms": text_value(reg.group(3)), "f": text_value(reg.group(4)),
                           "note": "f value null when NIST prints Infinity"},
            "residual": {"df": int(res.group(1)), "ss": text_value(res.group(2)),
                         "ms": text_value(res.group(3))},
        },
        "n_observations": len(rows),
        "data": data,
    }


def parse_anova(name):
    L = read(name)
    r = ranges(L)
    # the header's certified range stops short of the residual SD line
    txt = "\n".join(L[r["cert"][0] - 1:r["data"][0] - 1])
    bt = re.search(r"Between \w+\s+(\d+)\s+(%s)\s+(%s)\s+(%s)" % (NUM, NUM, NUM), txt)
    wt = re.search(r"Within \w+\s+(\d+)\s+(%s)\s+(%s)" % (NUM, NUM), txt)
    r2 = re.search(r"R-Squared\s+(%s)" % NUM, txt).group(1)
    rsd = re.search(r"Standard Deviation\s+(%s)" % NUM, txt).group(1)
    cols = header_cols(L, r["data"])
    rows = data_rows(L, r["data"], 2)
    return {
        "name": name, "class": "anova_one_way", "difficulty": difficulty(L, name),
        "source_url": "%s/anova/%s.dat" % (BASE, name),
        "columns": cols,
        "between": {"df": int(bt.group(1)), "ss": text_value(bt.group(2)),
                    "ms": text_value(bt.group(3)), "f": text_value(bt.group(4))},
        "within": {"df": int(wt.group(1)), "ss": text_value(wt.group(2)),
                   "ms": text_value(wt.group(3))},
        "r_squared": text_value(r2),
        "residual_std_dev": text_value(rsd),
        "n_observations": len(rows),
        "data": {"group": [int(x[0]) for x in rows], "y": [x[1] for x in rows]},
    }


def parse_univ(name):
    L = read(name)
    r = ranges(L)
    txt = "\n".join(L[r["cert"][0] - 1:r["data"][0] - 1])
    mean = re.search(r"ybar:\s*(%s)" % NUM, txt).group(1)
    sd = re.search(r"\bs:\s*(%s)" % NUM, txt).group(1)
    ac = re.search(r"r\(1\):\s*(%s)" % NUM, txt).group(1)
    n = int(re.search(r"Number of Observations:\s*(\d+)", txt).group(1))
    a, b = r["data"]
    vals = [fnum(ln) for ln in L[a - 1:b] if ln.strip()]
    assert len(vals) == n, (name, len(vals), n)
    return {
        "name": name, "class": "univariate", "difficulty": difficulty(L, name),
        "source_url": "%s/univ/data/%s.dat" % (BASE, name),
        "certified": {"mean": text_value(mean), "sd_n_minus_1": text_value(sd),
                      "lag1_autocorrelation": text_value(ac)},
        "exact_note": "(exact)" if "(exact)" in txt else None,
        "n_observations": n,
        "data": {"y": vals},
    }


def main():
    index = []
    for kind, names, fn in (("nonlinear_regression", NLS, parse_nls), ("linear_regression", LLS, parse_lls),
                            ("anova_one_way", ANOVA, parse_anova), ("univariate", UNIV, parse_univ)):
        for n in names:
            d = fn(n)
            d["dat_file"] = n + ".dat"
            with open(os.path.join(NIST, n + ".json"), "w") as fh:
                json.dump(d, fh, indent=1)
            with open(os.path.join(NIST, n + ".json")) as fh:
                json.load(fh)
            index.append({"name": n, "class": kind, "difficulty": d["difficulty"]})
            if d["difficulty"] is None:
                print("warning: no difficulty for", n, file=sys.stderr)
    with open(os.path.join(NIST, "index.json"), "w") as fh:
        json.dump(index, fh, indent=1)
    print("wrote %d datasets" % len(index))


if __name__ == "__main__":
    main()
