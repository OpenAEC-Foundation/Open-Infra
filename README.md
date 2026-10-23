# Open Infra — GIS for Open CAD Studio

Plugin for [Open CAD Studio](https://github.com/HakanSeven12/OpenCADStudio) for infrastructure and civil engineering (infra en civiele techniek), in the Netherlands and worldwide. The first module is a **GIS workbench** modelled on the FreeCAD GIS(2BIM) workbench. You set the project location and coordinate system, then pull map layers, vector data and OpenStreetMap straight into the DWG.

## In Open CAD Studio

A ribbon tab **"Open Infra — GIS"**, grouped like the toolbars of the FreeCAD GIS workbench:

| Group | Button | What it does |
|---|---|---|
| Webservices | **Geo-locatie** | Set the project location: click it on the map or search an address, place, parcel or `lat, lon` (PDOK Locatieserver + Nominatim), choose the coordinate system, draw or type the bounding box, choose absolute or relative coordinates. Optionally place a base-point marker (GeoOrigin) and import an aerial photo. Stored in the drawing (NOD XRecord `OPENINFRA/SITE`). |
| | **WMS / TMS** | Choose a server, browse the layers from GetCapabilities, **Test** shows a preview on the map, **OK en importeren** places IMAGE underlays (sent to the back). |
| | **WFS** | Choose a server and feature type, **Test** fetches the objects and draws them on the map, then label attribute, colour and clipping, **OK en importeren**. |
| | **OpenStreetMap** | Worldwide themes (buildings, roads, rail, water, civil structures, land use, cables/pipelines, fences, house numbers). **Test** shows them on the map; OK reuses that result. |
| | **PDOK (NL)** | Dutch base registers in one go: aerial photo, cadastral map, AHN, BAG, parcels, NWB road sections and hectometre markers, ProRail track axes and kilometre markers, waterways. |
| Snel importeren | **Kaartbeeld ▾ / Vectordata ▾** | One click, no window, using the saved location. |
| Hulpmiddelen | **Gebied** · **Punt info** · **Coördinaten** · **Servers** | Pick the import area in the drawing, read a point's coordinates, convert coordinates (also by clicking the map), manage presets and your own WMS/WFS/XYZ servers. |

### The workbench window

Every page follows the FreeCAD GIS dialogs:

1. A **map** at the top shows the project location and the bounding box.
2. Grouped settings follow: *Locatie / Begrenzing*, *Webserver / verzoek* and *Import-instellingen*.
3. A fixed action bar holds **OK en importeren**, **Test** and **Annuleren**.

The map is a slippy map with OpenStreetMap, PDOK BRT (standard and grey), PDOK aerial photo and Sentinel-2 backgrounds. You choose the background in a **Leaflet-style layer switcher** at the top right, which also toggles the bounding box and the import preview.

Controls on the map:

| Action | Effect |
|---|---|
| Drag | Pan |
| Wheel | Zoom |
| Click | Set the location |
| Shift+drag, or *Gebied tekenen* | Draw the bounding box |

### Coordinate systems

- Built in: RD New (EPSG:28992), ETRS89/UTM 31–33N, Belgian Lambert 72/2008, British National Grid, CH1903+/LV95, Lambert-93, WGS 84 and Web Mercator.
- **Automatic UTM** anywhere in the world.
- Any other **EPSG code** is fetched once from epsg.io and stored with the drawing.
- The address search proposes the national grid for the country it finds.
- RD uses the 7-parameter transformation, which is about 0.2 m from RDNAPTRANS. That is fine for underlays and context. For survey work, use data that is already in RD (PDOK supplies it that way).

### Drawing units

`INSUNITS` sets the drawing units: metres (default for civil drawings), mm or cm.

- **Absolute:** CRS coordinates go straight into the drawing, e.g. RD in metres, as usual in Dutch infra work.
- **Relative:** the base point becomes 0,0.

Map images are saved next to the drawing in `<name>_GIS/`, or in the plugin's config folder while the drawing has not been saved yet.

## Architecture

```
src/gis/      host-free engine: crs (proj4rs), site, servers, caps (WMS/WFS GetCapabilities),
              geocode, raster (GetMap/XYZ + reprojection), wfs, osm (Overpass), vector (GeoJSON + clipping)
src/emit.rs   engine output → DWG entities (LWPOLYLINE, TEXT, CIRCLE, IMAGE)
src/bridge.rs host requests from worker/window threads (PluginRequestSender), site in the DWG
src/import.rs import flows shared by the window and the ribbon quick buttons
src/ui/       workbench window (egui), runs in the plugin process beside the CAD window
src/lib.rs    manifest, ribbon, commands (GIS_*), click interaction (Gebied, Punt info)
```

Open CAD Studio runs plugins out-of-process and has no dialog API for plugins, so the plugin opens its own window. Network work runs on worker threads, so the CAD window stays responsive and the plugin call timeout is never reached.

## Building and installing

You need the same `rustc` and the same opencadcodec revision as the Open CAD Studio build that will load the plugin. The host checks both.

```bash
./scripts/install.sh      # cargo build --release + install into ~/.config/OpenCADStudio/plugins/openaec.open-infra
cargo test --lib          # engine tests (CRS, clipping, tiling, parsing)
```

`Cargo.toml` pins `ocs_plugin_api` to the **Open CAD Studio v2026.42** release (plugin API 10, opencadcodec `5d4f11a`, rustc 1.99.1). So the plugin loads in the Open CAD Studio you can download. For a newer Open CAD Studio, move the `rev` along to the commit of that release, and the rustc version in `.github/workflows/release.yml` with it.

For development against a local Open CAD Studio checkout, a `.cargo/config.toml` (not committed) can point to it:

```toml
[patch."https://github.com/HakanSeven12/OpenCADStudio"]
ocs_plugin_api = { path = "../OpenCADStudio/crates/ocs_plugin_api" }
```

### Installing a release

Use Open CAD Studio's **Plugin Manager** (`PLUGINS`): *Add a repository* → `OpenAEC-Foundation/Open-Infra` → install release `v0.2.0`. Or download `openaec.open-infra-<platform>` and `plugin.toml` from the GitHub release into `<config>/OpenCADStudio/plugins/openaec.open-infra/`, then restart Open CAD Studio.

Commands (for scripts and automation): `GIS_LOCATIE`, `GIS_GEBIED [x1,y1 x2,y2]`, `GIS_PUNT`, `GIS_KAART`, `GIS_WFS`, `GIS_OSM`, `GIS_COORD`, `GIS_SERVERS`, `GIS_HELP`, and the quick imports `GIS_Q_LUCHTFOTO`, `GIS_Q_KADASTER`, `GIS_Q_AHN`, `GIS_Q_OSMKAART`, `GIS_Q_SATELLIET`, `GIS_Q_BAG`, `GIS_Q_PERCELEN`, `GIS_Q_NWB`, `GIS_Q_SPOOR`, `GIS_Q_OSM`.

## Platforms

Linux (X11/Wayland) and Windows. On macOS the workbench window is not available yet, because winit needs the main thread there. The ribbon quick buttons and commands do work.

## Data sources and licences

- PDOK, Kadaster, Rijkswaterstaat and ProRail: open data. Credit the source.
- OpenStreetMap: © OpenStreetMap contributors (ODbL), via Overpass, Nominatim and the OSM tile server. Follow their usage policies (light, interactive use).
- Sentinel-2 cloudless by EOX: CC BY-NC-SA 4.0 (non-commercial).

Licence: GPL-3.0-only, the same as the Open CAD Studio plugin API.
