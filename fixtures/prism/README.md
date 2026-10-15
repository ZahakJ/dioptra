# Prism fixtures

Real GraphPad Prism files for the import tests in `crates/dioptra-core/tests/prism_import.rs`.
Every file is redistributable under the licence named below and is unmodified, except
`figshare34328751__ACTC1.prism`, renamed from `ACTC1过表达检测.prism`. The licence texts are
at the repository or dataset URL.

| File | Source | Licence | Author / repository | Covers |
|---|---|---|---|---|
| `Yue-Jiang__column_low-high.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/column_low-high.pzfx | MIT | Yue Jiang, [Yue-Jiang/pzfx](https://github.com/Yue-Jiang/pzfx) | `YFormat="low-high"` |
| `Yue-Jiang__column_upper-lower-limits.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/column_upper-lower-limits.pzfx | MIT | Yue Jiang, [Yue-Jiang/pzfx](https://github.com/Yue-Jiang/pzfx) | `YFormat="upper-lower-limits"` |
| `Yue-Jiang__x_date.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/x_date.pzfx | MIT | Yue Jiang, [Yue-Jiang/pzfx](https://github.com/Yue-Jiang/pzfx) | `XFormat="date"` |
| `Yue-Jiang__x_error_y_sdn.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/x_error_y_sdn.pzfx | MIT | Yue Jiang, [Yue-Jiang/pzfx](https://github.com/Yue-Jiang/pzfx) | `XFormat="error"`, `YFormat="SDN"` |
| `Yue-Jiang__x_y_with_strike.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/x_y_with_strike.pzfx | MIT | Yue Jiang, [Yue-Jiang/pzfx](https://github.com/Yue-Jiang/pzfx) | Excluded values, replicates |
| `Yue-Jiang__survival.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/survival.pzfx | MIT | Yue Jiang, [Yue-Jiang/pzfx](https://github.com/Yue-Jiang/pzfx) | Survival table |
| `Yue-Jiang__contingency.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/contingency.pzfx | MIT | Yue Jiang, [Yue-Jiang/pzfx](https://github.com/Yue-Jiang/pzfx) | Contingency table |
| `cheminfo__table.pzfx` | https://raw.githubusercontent.com/cheminfo/pzfx-parser/master/data/table.pzfx | MIT | cheminfo, [cheminfo/pzfx-parser](https://github.com/cheminfo/pzfx-parser) | `YFormat="CVN"` |
| `hammerlab__Fig2.pzfx` | https://raw.githubusercontent.com/hammerlab/t-cell-electroporation/master/Prism_files/Fig2.pzfx | Apache-2.0 | Hammer Lab, [hammerlab/t-cell-electroporation](https://github.com/hammerlab/t-cell-electroporation) | Nested tables, shared `<SubColumnTitles>` |
| `smestern__prism_template2.pzfx` | https://raw.githubusercontent.com/smestern/pyAPisolation/master/pyAPisolation/dev/prism_template2.pzfx | MIT | smestern, [smestern/pyAPisolation](https://github.com/smestern/pyAPisolation) | Multiple variables, per-data-set `<SubColumnTitles>`, four kinds |
| `taehoonh__cats_multiple.pzfx` | https://raw.githubusercontent.com/taehoonh/CSHLbiostat2024/main/files/lec5/cats_multiple.pzfx | MIT | taehoonh, [taehoonh/CSHLbiostat2024](https://github.com/taehoonh/CSHLbiostat2024) | Multiple variables with a categorical column |
| `figshare34328751__ACTC1.prism` | https://ndownloader.figshare.com/files/69873774 | CC-BY-4.0 | Chengpeng Cao, [figshare 34328751](https://figshare.com/articles/dataset/_i_ACTC1_i_governs_myoblast_proliferation_and_myogenic_differentiation_in_Rex_rabbits_by_activating_glycolytic_metabolism/34328751) | `.prism` format 1-2-0, column table |
| `figshare34469319__RTqPCR_MEC_2D_vs_3D__4_genes.prism` | https://ndownloader.figshare.com/files/70071333 | CC-BY-4.0 | Joao Ferreira, [figshare 34469319](https://figshare.com/articles/dataset/_b_High-grade_Salivary_Gland_Mucoepidermoid_Carcinoma_Tumoroids_for_Radiation_Cytotoxicity_Screening_b_/34469319) | `.prism` format 1-4-0, grouped and column tables |
| `figshare33793582__X_inactivation_graphs.prism` | https://ndownloader.figshare.com/files/69182731 | CC0-1.0 | Amy Dashwood, [figshare 33793582](https://figshare.com/articles/dataset/_b_Hprt1Cre_A_novel_somatic_chimeric_mouse_model_for_studying_competitive_cellular_kinetics_beyond_the_classical_haematopoietic_lineages_b_/33793582) | `.prism` format 1-6-0, rich-text titles |

## corpus/

More real files, imported as a smoke test (`every_fixture_imports`). Unmodified except
`zhuli1903121__subtypes.pzfx`, renamed from `亚表型.pzfx`.

| File | Source | Licence | Repository | Tables |
|---|---|---|---|---|
| `MBrancoLab__ENG.pzfx` | https://raw.githubusercontent.com/MBrancoLab/Frost_2022_hTroph/main/Assays/ENG.pzfx | MIT | https://github.com/MBrancoLab/Frost_2022_hTroph | XYx13 |
| `Merck__Prism_Plots_for_AbVax_manuscript.pzfx` | https://raw.githubusercontent.com/Merck/AbVax_Manuscript/main/Prism_Plots_for_AbVax_manuscript.pzfx | MIT | https://github.com/Merck/AbVax_Manuscript | OneWayx9 TwoWayx3 XYx13 |
| `Yue-Jiang__column.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/column.pzfx | MIT | https://github.com/Yue-Jiang/pzfx | OneWayx1 |
| `Yue-Jiang__column_empty.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/column_empty.pzfx | MIT | https://github.com/Yue-Jiang/pzfx | OneWayx1 |
| `Yue-Jiang__column_sdn.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/column_sdn.pzfx | MIT | https://github.com/Yue-Jiang/pzfx | TwoWayx1 |
| `Yue-Jiang__column_unequal_lengths.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/column_unequal_lengths.pzfx | MIT | https://github.com/Yue-Jiang/pzfx | OneWayx1 |
| `Yue-Jiang__comma_decimal.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/comma_decimal.pzfx | MIT | https://github.com/Yue-Jiang/pzfx | OneWayx1 |
| `Yue-Jiang__parts_of_whole.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/parts_of_whole.pzfx | MIT | https://github.com/Yue-Jiang/pzfx | PartsOfWholex1 |
| `Yue-Jiang__x_y_rep.pzfx` | https://raw.githubusercontent.com/Yue-Jiang/pzfx/main/inst/testdata/x_y_rep.pzfx | MIT | https://github.com/Yue-Jiang/pzfx | XYx1 |
| `datagrok-ai__dose-response-4pl.pzfx` | https://raw.githubusercontent.com/datagrok-ai/public/master/packages/Curves/files/dose-response-4pl.pzfx | MIT | https://github.com/datagrok-ai/public | XYx1 |
| `datagrok-ai__mixed-tables.pzfx` | https://raw.githubusercontent.com/datagrok-ai/public/master/packages/Curves/files/mixed-tables.pzfx | MIT | https://github.com/datagrok-ai/public | OneWayx1 XYx1 |
| `dhimmel__lung.pzfx` | https://raw.githubusercontent.com/dhimmel/elevcan/master/manual/figures/intermediate/lung.pzfx | CC0-1.0 (data) / CC-BY-4.0 (rest) | https://github.com/dhimmel/elevcan | TwoWayx2 |
| `ePBRN__Graph.pzfx` | https://raw.githubusercontent.com/ePBRN/AMT-Study-JAMIA/master/Analysis/Graph.pzfx | Apache-2.0 | https://github.com/ePBRN/AMT-Study-JAMIA | TwoWayx1 |
| `erenozen__sample.pzfx` | https://raw.githubusercontent.com/erenozen/opendose/main/web/e2e-fixtures/sample.pzfx | MIT | https://github.com/erenozen/opendose | OneWayx1 XYx1 |
| `figshare34469319__Radiation_assay_Ki67_PI_Annexin_V_Apoptosis.pzfx` | https://ndownloader.figshare.com/files/70071330 | CC-BY-4.0 | https://figshare.com/articles/dataset/_b_High-grade_Salivary_Gland_Mucoepidermoid_Carcinoma_Tumoroids_for_Radiation_Cytotoxicity_Screening_b_/34469319 | OneWayx7 |
| `hammerlab__Figs1-3-4-5-1600vs2200.pzfx` | https://raw.githubusercontent.com/hammerlab/t-cell-electroporation/master/Prism_files/Figs1-3-4-5-1600vs2200.pzfx | Apache-2.0 | https://github.com/hammerlab/t-cell-electroporation | Nestedx7 TwoWayx10 |
| `taehoonh__cats.pzfx` | https://raw.githubusercontent.com/taehoonh/CSHLbiostat2024/main/files/lec5/cats.pzfx | MIT | https://github.com/taehoonh/CSHLbiostat2024 | XYx1 |
| `zhuli1903121__subtypes.pzfx` | https://raw.githubusercontent.com/zhuli19031218/SepsisFormer/main/figure/4a4c/4c/亚表型.pzfx | MIT | https://github.com/zhuli19031218/SepsisFormer | XYx1 |
