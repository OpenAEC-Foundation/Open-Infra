# Open Infra outside the Netherlands, GIS files and the vehicle library

What the branch `internationaal-voertuigen` adds. Every part works anywhere in the world; the Dutch parts (NLCS, BGT, KLIC, AHN, GWSW) stay as they were.

## Interface language

Dutch or English. The choice is `language` in `<config>/OpenCADStudio/open-infra/settings.json` (`"nl"` or `"en"`). Without that setting, the system locale decides: `nl*` gives Dutch, anything else English.

The workbench window has a switch under the navigation. It takes effect at the next start of Open CAD Studio, because the ribbon is built when the plugin loads.

## Web services in 52 countries

The server lists on the raster and WFS pages hold 600 extra presets:
- 426 WMS, 131 WFS and 43 XYZ tile services;
- from national, regional and city geoportals in 52 countries, plus pan-European and worldwide sources.

Each one answered GetCapabilities and a sample request when the list was checked, and none needs an API key.

A filter at the top of the list finds a country, region or name.

The list comes from the checked web service catalogue of GIS2BIM (branch `worldwide-servers`). `tools/import_gis2bim_servers.py` rebuilds `assets/servers/world.json` from it.

## Heights everywhere

Spot heights, contour lines and length profiles work at any location:

| Where | Source | Grid | Reference |
|---|---|---|---|
| Netherlands | AHN4 (PDOK WCS) | RD, 0.5 m | NAP |
| Elsewhere | [Terrain Tiles](https://registry.opendata.aws/terrain-tiles/) (AWS Open Data) | the site's CRS (UTM when the site is in degrees), 5 m or coarser | about mean sea level |

The Terrain Tiles combine SRTM, GMTED and ETOPO1 with finer national models where those were open, such as USGS 3DEP and Austria. They need no API key.

Outside the Netherlands the contour interval is at least 1 m: finer intervals on 30 m data only draw noise. The AHN point cloud stays Netherlands only.

`gis::terrain::Source` chooses the source and gives the grid, the profile stations, the label, the attribution and the height reference.

## GIS files (issue #7)

**GIS-bestand / GIS file** in the ribbon opens a file dialog, then the workbench page. These formats are read:

| Format | CRS from |
|---|---|
| GeoJSON | the `crs` member, otherwise WGS 84 (RFC 7946) |
| Shapefile (`.shp` + `.dbf` + `.prj`) | the `.prj`: EPSG authority or a known ESRI name (RD_New, UTM zones, ...) |
| GeoPackage | `gpkg_spatial_ref_sys`; a layer choice when there are several |
| KML, KMZ | WGS 84 |
| GPX | WGS 84 (waypoints, routes, tracks) |
| CSV / TXT | `lon/lat` columns → WGS 84; `x/y`, `easting/northing`, `x_rd/y_rd` → the site's CRS (check it) |

Options:
- each feature's attributes go on its entities as XDATA `OPENINFRA` (up to 40);
- one layer per value of an attribute;
- labels from an attribute;
- text height and colour;
- clipping to the import area.

Commands: `GIS_BESTAND | GIS_FILE [path]` (window) and `GIS_BESTAND_IMPORT | GIS_FILE_IMPORT <path> [EPSG:code]` (straight in).

## GWSW sewers (issue #6)

`GIS_Q_GWSW` fetches the GWSW collections from the PDOK OGC API (`beheer-stedelijk-watersystemen-gwsw`, CC0): pipes, connection pipes, manholes, structures, pumps and connection points.

Each object goes on the most specific NLCS RI object, chosen in this order:
1. the system (GWA, HWA, DWA, ITR, PERSLEIDING, DRAINAGE), from the GWSW type or the system type;
2. the material;
3. the diameter.

An example layer is `B-XX-RI-GWA_RIOOLLEIDING_BETON_600-G`.

Manholes carry their name on the PUTNUMMER object, and every object carries its GWSW attributes as XDATA. Not every municipality publishes its network.

## GEODATA (issue #8, reading)

A drawing from AutoCAD, Civil 3D or BricsCAD with a geographic location (GEOGRAPHICLOCATION, GEOMAP) and no Open Infra site takes its site from the GEODATA object:
- **CRS:** from the MapGuide XML or the WKT;
- **absolute or relative:** absolute when the design point equals the reference point.

Writing GEODATA needs Open CAD Studio to offer it: the plugin API v2026.42 creates no objects other than XRecords.

## Vehicle library

**Voertuigen / Vehicles** in the ribbon opens the library:
- search by name or standard;
- filter by category and region;
- see the vehicle in plan or side view;
- insert it into the drawing.

Each vehicle becomes a block, `OI_<ID>_TOP` or `OI_<ID>_SIDE`:
- **block geometry:** on layer 0, ByBlock;
- **inserts:** on `OI-VOERTUIGEN` / `OI-VEHICLES`, with the dimensions, standard and source as XDATA;
- **base point:** the centre of the front; the vehicle drives along +X.

| Command | What |
|---|---|
| `GIS_VOERTUIG \| GIS_VEHICLE <id> [ZIJ\|SIDE]` | pick the front, then the driving direction (Enter: east). A side view picked to the left is mirrored. |
| `GIS_VOERTUIGOVERZICHT \| GIS_VEHICLE_CATALOG [category] [ZIJ\|SIDE]` | a catalogue sheet: rows per category, with names and dimensions |
| `GIS_VOERTUIGEN \| GIS_VEHICLES` | the window |

The data is `assets/library/vehicles.json`. Each vehicle has:
- its units (tractor and semitrailer, bus and rear section, ...), each with its length and width;
- per unit, its axles with their positions, track, twin tyres and steering;
- the coupling points between units;
- the standard and the source page;
- which values were estimated.

## Swept paths

`GIS_VEEGPAD | GIS_SWEPT_PATH <id> [interval]` drives a vehicle along a selected polyline, or along picked points ended with Enter:
- the steer axle follows the line;
- each rear axle and each trailer follows as a tractrix (the low-speed model of swept-path software).

Drawn:

| Layer | What |
|---|---|
| `OI-VEEGPAD` / `OI-SWEPT-PATH` | the traces of every unit's body corners |
| `OI-VEEGPAD-WIELEN` / `-WHEELS` | the outer wheel edges |
| `OI-VEEGPAD-VOERTUIG` / `-VEHICLE` | the vehicle every interval (default 10 m); each unit is its own block (`OI_<ID>_U<n>_TOP`), so they turn apart |

The message gives the largest steering angle and the smallest outer radius. It warns when the path is tighter than the vehicle's design turning radius.

## Tests

- `cargo test --lib` covers the engine:
  - vehicle data and drawings, swept paths;
  - Terrain Tiles decoding;
  - every file format, using `tests/data/gisbestanden`, made by `maak.py`;
  - GWSW → NLCS mapping;
  - GEODATA definitions.
- `python3 tests/e2e/bibliotheek_e2e.py` runs its own Open CAD Studio with an isolated configuration folder. In Zürich it imports the six file formats, inserts vehicles and a catalogue, and draws contour lines and spot heights.
