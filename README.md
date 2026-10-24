# Open Infra — GIS and NLCS base data for Open CAD Studio

[![Release](https://img.shields.io/github/v/release/OpenAEC-Foundation/Open-Infra?label=release)](https://github.com/OpenAEC-Foundation/Open-Infra/releases/latest)
[![Open CAD Studio](https://img.shields.io/badge/Open%20CAD%20Studio-v2026.42-2196dc)](https://github.com/HakanSeven12/OpenCADStudio)
[![Plugin API](https://img.shields.io/badge/plugin%20API-10-6db7ed)](https://github.com/HakanSeven12/OpenCADStudio/tree/master/crates/ocs_plugin_api)
[![NLCS](https://img.shields.io/badge/NLCS-5.02-98c379)](https://www.nlcs.nl)
[![Licence](https://img.shields.io/badge/licence-GPL--3.0--only-lightgrey)](#data-sources-and-licences)

Plugin for [Open CAD Studio](https://github.com/HakanSeven12/OpenCADStudio) for infrastructure and civil engineering (infra en civiele techniek), in the Netherlands and worldwide.

- **GIS workbench**, modelled on the FreeCAD GIS(2BIM) workbench. Set the project location and coordinate system, then pull aerial photos, WMS/WFS layers and OpenStreetMap straight into the DWG.
- **Dutch base data conform NLCS**, like InfraCAD Map: BGT, cadastral map and KLIC deliveries on NLCS layers, with object data and a finished drawing sheet.
- **AHN heights**: spot heights, contour lines and length profiles from AHN4, and the AHN5 point cloud attached in 3D.

![Example: situatie bestaand around the Grote Kerk in Dordrecht, A1 1:500](examples/dordrecht-nlcs/dordrecht-grote-kerk-nlcs.png)

<sub>[Example drawing](examples/dordrecht-nlcs): BGT, cadastre and a KLIC delivery around the Grote Kerk in Dordrecht. A1 at 1:500, made with five commands. Available as DWG, PDF and PNG.</sub>

## At a glance

| | |
|---|---|
| **NLCS 5.02 built in** | 3,659 objects in 32 main groups, 10 disciplines, 4 drawing statuses, 256 colours |
| **NLCS linetypes** | 543 (107 plain dash patterns, 436 with symbols or text) |
| **BGT → NLCS** | 719 rules from the official NLCS-BGT 5.0 mapping |
| **BGT** | 42 PDOK collections in 11 groups, current objects only |
| **KLIC** | 13 IMKL 2.0 themes: electricity (LS/MS/HS), data, gas (LD/HD), water, district heating, sewer, chemicals |
| **Server presets** | 19: 10 WMS, 8 WFS and 1 XYZ, covering the Netherlands (12), Flanders (2) and worldwide (5), plus your own servers |
| **One-click imports** | 11 PDOK datasets, 11 OpenStreetMap themes, 11 quick buttons |
| **AHN** | AHN4 DTM/DSM at 0.5 m (PDOK WCS); AHN4/AHN5 point clouds (COPC), only the octree nodes of the project area are downloaded |
| **Coordinate systems** | 14 built in, automatic UTM worldwide, any EPSG code via epsg.io |
| **Map backgrounds** | 5: OpenStreetMap, PDOK BRT (standard and grey), PDOK aerial photo, Sentinel-2 |
| **Commands** | 33 `GIS_*` commands for the ribbon, scripts and automation |
| **Code** | about 6,900 lines of Rust, 17 unit tests; Linux, Windows and macOS builds |

## In Open CAD Studio

![Open CAD Studio with the Open Infra ribbon tab and the example drawing](docs/images/open-cad-studio-open-infra.png)

The ribbon tab **"Open Infra — GIS"**:

| Group | Button | What it does |
|---|---|---|
| Webservices | **Geo-locatie** | Set the project location. Click it on the map, or search an address, place, parcel or `lat, lon` (PDOK Locatieserver and Nominatim). Choose the coordinate system, draw or type the bounding box, and choose absolute or relative coordinates. Optionally place a base-point marker and import an aerial photo. The location is stored in the drawing (NOD XRecord `OPENINFRA/SITE`). |
| | **WMS / TMS** | Choose a server and browse its layers. **Test** shows a preview on the map; **OK en importeren** places IMAGE underlays behind the drawing. |
| | **WFS** | Choose a server and feature type. **Test** draws the objects on the map. Then set the label attribute, colour and clipping. |
| | **OpenStreetMap** | Worldwide themes: buildings, roads, rail, water, civil structures, land use, cables and pipelines, fences, house numbers. |
| | **PDOK (NL)** | Several Dutch base registers in one go: aerial photo, cadastral map, AHN, BAG, parcels, NWB road sections and hectometre markers, ProRail tracks and kilometre markers, waterways. |
| Snel importeren | **Kaartbeeld ▾ / Vectordata ▾** | One click, no window, using the saved location. |
| Basisdata NL (NLCS) | **BGT · BRK · KLIC** | The NLCS page of the workbench (see below). |
| | **BGT** · **Kadaster** | One click: the current BGT, or cadastral boundaries and parcel numbers, on NLCS layers. |
| | **KLIC** | Open a KLIC delivery: the Kadaster zip or `GI_gebiedsinformatielevering_*.xml`. It is shown on the map, then drawn. |
| | **Tekenblad** | NLCS drawing sheet around the import area: frame, title block, legend of the layers used, north arrow, scale bar and RD coordinate crosses. |
| Hoogte (AHN) | **AHN** | The AHN page of the workbench: options, and **Test** to show contour lines on the map. |
| | **Hoogtepunten** · **Hoogtelijnen** | Spot heights in a grid, and contour lines (minor/major, labelled) from AHN4, at their true elevation. |
| | **Profiel** | Length profile along a selected polyline or picked points: ground (DTM) and optionally surface (DSM), height grid, profile band with distances and heights, and section line A–A' in plan. |
| | **Puntenwolk** | The AHN5 (or AHN4) point cloud of the import area, coloured by class, height or RGB. It is written as `.rcs` next to the drawing and attached; it shows in a 3D visual style. |
| Hulpmiddelen | **Gebied** · **Punt info** · **Coördinaten** · **Servers** | Pick the import area in the drawing, read a point's coordinates, convert coordinates, and manage presets and your own WMS/WFS/XYZ servers. |

## The workbench window

Every page follows the FreeCAD GIS dialogs:

1. A **map** at the top shows the project location and the bounding box.
2. Grouped settings follow.
3. A fixed action bar holds **OK en importeren**, **Test** and **Annuleren**.

You choose the map background in a **Leaflet-style layer switcher** at the top right. Click the map to set the location; Shift+drag draws the bounding box.

| Geographic location | WMS preview (PDOK aerial photo, 8 cm) |
|---|---|
| ![Location page](docs/images/workbench-location.png) | ![WMS page with preview](docs/images/workbench-wms-preview.png) |
| **Server presets**, Netherlands, Flanders and worldwide | **BGT · Kadaster · KLIC** with the BGT preview in NLCS colours |
| ![Server list](docs/images/workbench-servers.png) | ![NLCS page](docs/images/workbench-nlcs.png) |

## Dutch base data conform NLCS

The NLCS tables and the official NLCS-BGT 5.0 mapping are built in from [`assets/nlcs`](assets/nlcs): objects, colours, linetypes, statuses, disciplines and main groups. The source is DigiGO / Stichting NLCS, under CC BY 4.0. `tools/nlcs_tables.py` regenerates them from a new NLCS publication.

- **Layers:** every object goes on its NLCS layer `STATUS-DISCIPLINE-HOOFDGROEP-OBJECT-ELEMENT`, e.g. `B-XX-VH-GESLOTENVERHARDING_ASFALT-G` or `B-OI-KL-GAS_LD-G`.
  - The layer gets the NLCS colour, lineweight and linetype for the chosen drawing status (B/N/T/V).
  - Linetypes are defined in paper millimetres, and entities get the linetype scale of the plot scale.
  - Elements: G for lines and areas, S for points, T for texts.
- **BGT:** current objects from the PDOK OGC API (`eind_registratie IS NULL`). Each object is mapped to the most specific NLCS-BGT rule, using function, physical appearance and plus attributes. Street names and house numbers are placed at their BGT rotation.
- **Cadastral map (BRK):** parcel boundaries on `KG-GRENS_PERCEEL`, and parcel numbers at their BRK rotation.
- **KLIC:**
  - IMKL 2.0 deliveries are read with OpenAEC's [`klic2geo`](https://github.com/OpenAEC-Foundation/crates-warehouse).
  - Each theme goes on its KL or RI object.
  - Cables and pipes get labels such as `G-LD Stedin 160 mm PVC`, with duplicates on parallel cables left out.
  - IMKL theme colours can be used instead of the NLCS colours.
- **Object data:** each entity carries its source attributes as XDATA `OPENINFRA`, for example BGT `lokaal_id`, function and material, or the KLIC operator, diameter and material.
- **Drawing sheet:**
  - The sheet format is chosen automatically (A3–A0 landscape).
  - The title block lists the data sources of the imports.
  - The legend shows the NLCS layers used, grouped by main group.
  - Sheet texts follow the NLCS text classes (T1.8, T2.5, …).

## AHN heights, profiles and point clouds

- **Heights:** AHN4 from the PDOK WCS: DTM (ground level) and DSM (surface), 0.5 m float grids in m NAP. Larger areas are fetched in tiles or at a coarser cell size.
- **Spot heights** go on `OG-HOOGTEPUNT`, with labels such as `+2,43`. **Contour lines** go on `OG-HOOGTELIJN_MINOR/MAJOR` (marching squares on a lightly smoothed grid). Both sit at their real height (Z).
- **Length profile (`GIS_PROFIEL`):**
  - Select a polyline or line, or pick points and press Enter. Then pick the lower-left corner of the profile.
  - The profile draws the ground line on `GW-MAAIVELD`, optionally the surface line, and a height grid with NAP labels.
  - The profile band (`AL-PROFIELBALK`) lists distances and heights. The NAP line is drawn when it is in range, and the section line A–A' in plan.
  - The profile breaks where AHN has no ground level (buildings, water).
- **Point cloud (`GIS_PUNTENWOLK`):**
  - AHN5 or AHN4 is read as COPC (Cloud Optimized Point Cloud) from Het Waterschapshuis' 1 km tiles. Only the octree nodes in the import area are downloaded, at the chosen point spacing.
  - The cloud is coloured by class (ground, buildings, water, structures, other), by height or by RGB.
  - It is saved as `.rcs` next to the drawing and attached with POINTCLOUDATTACH.
  - Example: 300 × 250 m at 0.25 m spacing is about 1.4 million points and a 29 MB file.
- **Punt info** also reports the AHN ground and surface height of the picked point.

| Point cloud (AHN5, by class) with BGT and the drawing sheet | Length profile across the Grotekerksplein |
|---|---|
| ![AHN5 point cloud](docs/images/ahn-pointcloud.png) | ![Length profile](docs/images/ahn-profile.png) |

## Example drawing

[`examples/dordrecht-nlcs`](examples/dordrecht-nlcs) is the existing situation around the Grote Kerk in Dordrecht. It is 300 × 250 m in RD, on an A1 sheet at 1:500.

| | |
|---|---|
| Entities | 3,439: 1,242 BGT, 1,328 cadastre, 632 KLIC, 237 drawing sheet |
| NLCS layers | 68, with 8 NLCS linetypes |
| With object data (XDATA) | 3,202 |
| Files | DWG (0.7 MB), PDF A1 (1 MB), PNG |

| KLIC delivery around the church, labelled per operator | Legend and title block |
|---|---|
| ![KLIC detail](docs/images/example-klic-detail.png) | ![Legend](docs/images/example-legend.png) ![Title block](docs/images/example-title-block.png) |

It was made in an empty metric drawing with:

```
GIS_SITE RD 104885 425320 300 250 Dordrecht, Grote Kerk
GIS_Q_BGT
GIS_Q_BRK
GIS_KLIC_IMPORT <path>/Levering_21O072584_1.zip
GIS_TEKENBLAD Situatie bestaand - BGT, kadaster en kabels en leidingen
```

## Commands

For the command line, scripts and automation:

| Purpose | Commands |
|---|---|
| Workbench pages | `GIS_LOCATIE`, `GIS_KAART`, `GIS_WFS`, `GIS_OSM`, `GIS_PDOK`, `GIS_BGT`, `GIS_COORD`, `GIS_SERVERS` |
| In the drawing | `GIS_GEBIED` (pick the import area), `GIS_PUNT` (point info) |
| Location | `GIS_SITE RD <x> <y> [width] [height] [name]`, `GIS_SITE <lon> <lat> [width] [height] [EPSG:code] [name]` |
| Map images | `GIS_Q_LUCHTFOTO`, `GIS_Q_KADASTER`, `GIS_Q_AHN`, `GIS_Q_OSMKAART`, `GIS_Q_SATELLIET` |
| Vector data | `GIS_Q_BAG`, `GIS_Q_PERCELEN`, `GIS_Q_NWB`, `GIS_Q_HECTO`, `GIS_Q_SPOOR`, `GIS_Q_OSM` |
| NLCS base data | `GIS_Q_BGT`, `GIS_Q_BRK`, `GIS_KLIC [file]` (opens the page), `GIS_KLIC_IMPORT <file>`, `GIS_TEKENBLAD [subject]` |
| AHN | `GIS_AHN` (page), `GIS_HOOGTEPUNTEN`, `GIS_HOOGTELIJNEN`, `GIS_PROFIEL`, `GIS_PUNTENWOLK` |
| Help | `GIS_HELP` |

## Coordinate systems and units

- **Built in:**
  - RD New (EPSG:28992)
  - ETRS89 / UTM 31–33N
  - Belgian Lambert 72 and 2008
  - British National Grid
  - CH1903+ / LV95
  - Lambert-93
  - WGS 84 and Web Mercator
- **Automatic UTM** anywhere in the world.
- **Any other EPSG code** is fetched once from epsg.io and stored with the drawing.
- The address search proposes the national grid for the country it finds.
- **RD accuracy:** RD uses the 7-parameter transformation, about 0.2 m from RDNAPTRANS. That is fine for underlays and context. For survey work, use data that is already in RD; PDOK, BGT, BRK and KLIC all supply it that way.
- **Units:** `INSUNITS` sets the drawing units: metres (default for civil drawings), mm or cm.
  - **Absolute:** CRS coordinates go straight into the drawing, as is usual in Dutch infra work.
  - **Relative:** the base point becomes 0,0.
- **Map images** are saved next to the drawing in `<name>_GIS/`.

## Architecture

```
src/gis/          host-free engine: crs (proj4rs), site, servers, caps (WMS/WFS GetCapabilities),
                  geocode, raster (GetMap/XYZ + reprojection), wfs, osm (Overpass), vector (GeoJSON + clipping),
                  ogcapi (OGC API Features), bgt (PDOK BGT), klic (IMKL via klic2geo),
                  ahn (WCS height grids, contours, profiles), pointcloud (COPC over HTTP, .rcs writer)
src/nlcs.rs       NLCS 5.02 tables, layer names and styles, BGT/KLIC → NLCS mapping
src/basisdata.rs  BGT, cadastre and KLIC drawn conform NLCS, with object data
src/tekenblad.rs  NLCS drawing sheet: frame, title block, legend, north arrow, scale bar
src/hoogte.rs     AHN spot heights, contour lines, length profiles and point clouds in the drawing
src/emit.rs       engine output → DWG entities (LWPOLYLINE, TEXT, CIRCLE, IMAGE)
src/bridge.rs     host requests from worker/window threads, NLCS layers and linetypes, site in the DWG
src/import.rs     import flows shared by the window and the ribbon quick buttons
src/ui/           workbench window (egui), runs in the plugin process beside the CAD window
src/lib.rs        manifest, ribbon, commands (GIS_*), click interaction (Gebied, Punt info)
```

Open CAD Studio runs plugins out-of-process and has no dialog API for plugins, so the plugin opens its own window. Network work runs on worker threads, so the CAD window stays responsive and the plugin call timeout is never reached.

## Installing

Use Open CAD Studio's **Plugin Manager** (`PLUGINS`): choose *Add a repository*, enter `OpenAEC-Foundation/Open-Infra` and install the [latest release](https://github.com/OpenAEC-Foundation/Open-Infra/releases/latest).

Or install by hand:
1. Download `openaec.open-infra-<platform>` and `plugin.toml` from the release.
2. Put them in `<config>/OpenCADStudio/plugins/openaec.open-infra/`.
3. Restart Open CAD Studio.

Release numbers follow Open CAD Studio's calendar numbering. Release 2026.43 is built for Open CAD Studio v2026.42 (plugin API 10, opencadcodec `5d4f11a`, rustc 1.99.1). The host only loads a plugin built against its own opencadcodec revision and rustc.

## Building

```bash
./scripts/install.sh      # cargo build --release + install into ~/.config/OpenCADStudio/plugins/openaec.open-infra
cargo test --lib          # engine tests (CRS, clipping, tiling, parsing, NLCS tables)
```

`Cargo.toml` pins `ocs_plugin_api` to the Open CAD Studio v2026.42 release. For a newer Open CAD Studio, move the `rev` to the commit of that release, and update the rustc version in `.github/workflows/release.yml` to match.

For development against a local Open CAD Studio checkout, add a `.cargo/config.toml` (not committed):

```toml
[patch."https://github.com/HakanSeven12/OpenCADStudio"]
ocs_plugin_api = { path = "../OpenCADStudio/crates/ocs_plugin_api" }
```

## Platforms

- **Linux** (X11/Wayland) and **Windows**: everything works.
- **macOS**: the ribbon quick buttons and commands work. The workbench window does not yet, because winit needs the main thread there.

## Data sources and licences

- PDOK, Kadaster, Rijkswaterstaat and ProRail: open data (BGT CC0, BRK CC BY 4.0). Credit the source; the title block does this for the imports.
- AHN (Actueel Hoogtebestand Nederland): open data (CC0). Height grids come via the PDOK WCS, point clouds from Het Waterschapshuis' object storage.
- NLCS tables and the NLCS-BGT mapping: DigiGO / Stichting NLCS, CC BY 4.0 (see [`assets/nlcs/NOTICE.md`](assets/nlcs/NOTICE.md)).
- OpenStreetMap: © OpenStreetMap contributors (ODbL), via Overpass, Nominatim and the OSM tile server. Follow their usage policies (light, interactive use).
- Sentinel-2 cloudless by EOX: CC BY-NC-SA 4.0 (non-commercial).

Licence: GPL-3.0-only, the same as the Open CAD Studio plugin API.
