# Methods and report

*Dioptra Lab.* The one-line methods text on each results sheet is free.

## Methods paragraph

**Methods paragraph…** on a results sheet copies a paragraph for that analysis;
**File → Copy methods paragraph…** covers every analysis in the project. For each analysis
it names the data table, the test, sidedness, the correction for multiple comparisons and
the post tests, the confidence level and the data form, then the software and version, and
lists the references to cite once each. References are those of the methods Dioptra
implements, as listed in [Methods and references](analyses/METHODS.md).

```text
Plasma IL-6: Ordinary one-way ANOVA, Tukey's multiple comparisons (family-wise
α = 0.05). Analyses were performed in Dioptra 0.4.0 (https://github.com/ZahakJ/dioptra).

References: Fisher (1925); Tukey (1953); Kramer (1956), doi:10.2307/3001469.
```

## Report

**File → Export report…** writes the project as one document:

1. the title, the Dioptra version and the date;
2. **Data**: each table with its first 20 rows;
3. **Methods**: the methods paragraph;
4. **Results**: each results sheet, with its summary sentences, methods line, assumption
   checks and tables, p values in the project's style;
5. **Figures**: every graph and layout, numbered, with its title as the caption.

| Format | Pages | Figures |
|---|---|---|
| Word (`.docx`) | A4, 20 mm margins, Arial; headings use Word's heading styles | PNG at 300 dpi, scaled to the text width |
| PDF | A4, 20 mm margins, Inter | Vectors with embedded fonts; graphs wider than the text are laid out again at its width |

Long tables continue on the next page with their header repeated.
