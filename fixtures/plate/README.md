# Plate-reader fixtures

Sample exports for the parser tests in `crates/dioptra-core/tests/plate_import.rs`. All come
from the test data of Benchling's [allotropy](https://github.com/Benchling-Open-Source/allotropy)
(`tests/parsers/*/testdata/`), are unmodified apart from the file names, and are used under its
MIT licence:

> MIT License
>
> Copyright (c) 2023 Benchling, Inc.
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software
> and associated documentation files (the "Software"), to deal in the Software without
> restriction, including without limitation the rights to use, copy, modify, merge, publish,
> distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
> Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
> BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

| File | Original (`tests/parsers/…/testdata/`) | Covers |
|---|---|---|
| `softmax_abs_endpoint_partial_plate.txt` | `moldev_softmax_pro/MD_SMP_absorbance_endpoint_partial_plate_example05.txt` | UTF-16LE, PlateFormat, three wavelengths side by side, partial plate |
| `softmax_fl_kinetic.txt` | `moldev_softmax_pro/fl_kinetic_plates.txt` | UTF-8, kinetic PlateFormat, two wavelengths, 6-well plate |
| `softmax_lum_timeformat.txt` | `moldev_softmax_pro/MD_SMP_luminescence_endpoint_example03.txt` | TimeFormat endpoint, two plates |
| `bmg_abs_96.csv` | `bmg_mars/BMG Labtech format.CSV` | 96-well absorbance |
| `bmg_abs_384.csv` | `bmg_mars/16-03-03 16-54-03 472 ABS 384 QC.csv` | 384-well absorbance, empty wells |
| `gen5_abs_endpoint.txt` | `agilent_gen5/absorbance/endpoint_stdcurve_singleplate.txt` | Results grid with three labels per row |
| `gen5_two_reads.txt` | `agilent_gen5/multi_read_modes/two_same_read_modes.txt` | Two reads interleaved in one grid |
| `gen5_abs_kinetic.txt` | `agilent_gen5/absorbance/kinetic_helper_gene_growth_curve.txt` | Long-format kinetic, 24 wells |
| `magellan_od.xlsx` | `tecan_magellan/SLA01P01_OD.xlsx` | List layout, one read, 96 wells |
| `magellan_elisa.xlsx` | `tecan_magellan/SLA01P01_ELISA.xlsx` | List layout, two labels, 384 wells |
