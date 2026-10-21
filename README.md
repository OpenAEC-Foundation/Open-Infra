# Open Infra — GIS

Extension for Open 2D Studio / Open CAD Studio aimed at the infrastructure and civil engineering sector (infra en civiele techniek). Its first module is a **GIS workbench**, modelled on the FreeCAD GIS/GIS2BIM workbench. It works for the Netherlands (PDOK) and worldwide.

## Features (ribbon tab "GIS")

| Group | Function |
|---|---|
| **Locatie** | Search for an address, place, parcel or `lat, lon` (PDOK Locatieserver + OpenStreetMap Nominatim). Choose a coordinate reference system: RD New (EPSG:28992), Belgian Lambert 72/2008, BNG, LV95, Lambert-93, automatic UTM anywhere in the world, or any EPSG code via epsg.io. Set the base point and import extent, and choose relative or absolute coordinates. |
| **Coördinaten** | Convert coordinates between systems, read them from the selection, place a marked point. |
| **Kaartlagen** | WMS layers and XYZ tiles as an underlay (aerial photo, cadastral map, AHN, TOP10NL, OSM, Sentinel-2, GEBCO…). Large areas are split into tiles. Layers that don't offer the project CRS are reprojected client-side. |
| **Vectordata** | WFS → polylines, points and labels per layer (BAG, cadastral parcels, NWB roads/waterways, ProRail track, Natura 2000, Flanders GRB…). Pages beyond the server limit and clips to the import area. |
| **OpenStreetMap** | Worldwide: buildings, roads, rail, water, civil structures, land use, cables/pipelines, fences, house numbers. Each theme goes on its own layer. |
| **Serverbeheer** | Manage presets and add your own WMS/WFS/XYZ servers (tested with GetCapabilities). |

Drawing units are millimetres: 1 m in the CRS = 1000 mm. By default the base point is at 0,0 so coordinates stay small. You can switch to absolute coordinates per project.

## Development

```bash
npm install
npm run typecheck
npm run build            # dist/main.js (runtime extension)
DEPLOY=1 npm run build   # also install into the desktop app's extensions folder
npm run package          # dist/open-infra-extension.zip
```

If this repo sits next to `open-2d-studio/`, the app's Vite config bundles the extension directly from `../Open-Infra/src`. Run `npm run dev` / `npm run build` / `npm run tauri build` in `open-2d-studio`.

## Network and CORS

In the desktop app, requests go through the Tauri HTTP plugin. That only works for hosts the app's capabilities allow; otherwise the extension falls back to the webview's `fetch`. That fallback (and the browser build in general) only works with servers that send CORS headers. All presets do (PDOK, Digitaal Vlaanderen, terrestris, EOX, NASA GIBS, GEBCO, OSM, Overpass, Nominatim, epsg.io).

## Data sources and licences

PDOK/Kadaster/Rijkswaterstaat/ProRail: open data (CC0/CC BY). OpenStreetMap: © OpenStreetMap contributors, ODbL. Credit the source when you use the data. Sentinel-2 cloudless by EOX: CC BY-NC-SA 4.0 (non-commercial). Follow the usage policies of the OSM tile server, Nominatim and Overpass (light, interactive use).
