Open Infra — GIS-workbench voor **Open CAD Studio v2026.42** (plugin API 10).

**Installeren:** Open CAD Studio → `PLUGINS` → *Add a repository* → `OpenAEC-Foundation/Open-Infra` → installeer deze release en herstart.
Of handmatig: zet `openaec.open-infra-<platform>` en `plugin.toml` in `<config>/OpenCADStudio/plugins/openaec.open-infra/`.

**Nieuw in 0.2.0**
- Lint-tab *Open Infra — GIS*: Locatie, Gebied, Punt info, WMS/Tegels, WFS, OpenStreetMap, Coördinaten, Servers en snelle imports (PDOK luchtfoto, kadastrale kaart, AHN, BAG, percelen, NWB, ProRail, OSM, Sentinel-2).
- Werkbankvenster naar het voorbeeld van de FreeCAD GIS(2BIM)-workbench (Linux en Windows; op macOS werken de lintknoppen en commando's).
- Coördinatenstelsels: RD New, ETRS89/UTM, België, VK, Zwitserland, Frankrijk, automatisch UTM wereldwijd, elke EPSG-code via epsg.io.
- WMS met tegelopdeling en herprojectie, XYZ-tegels, WFS met paging, OpenStreetMap via Overpass, bijsnijden op het importgebied.
- Projectlocatie opgeslagen in de DWG; kaartbeelden naast de tekening en achter de vectoren; elke import is één ongedaan-maakstap.

Data: PDOK/Kadaster/Rijkswaterstaat/ProRail (open data), © OpenStreetMap-bijdragers (ODbL), Sentinel-2 cloudless by EOX (CC BY-NC-SA 4.0).
