Open Infra — GIS-workbench voor **Open CAD Studio v2026.42** (plugin-API 10), voor infra en civiele techniek in Nederland en wereldwijd.

**Installeren:** in Open CAD Studio `PLUGINS` → *Add a repository* → `OpenAEC-Foundation/Open-Infra`. Installeer deze release en herstart.
Handmatig kan ook: zet `openaec.open-infra-<platform>` en `plugin.toml` in `<config>/OpenCADStudio/plugins/openaec.open-infra/`.

**Nieuw in 0.2.0**
- **Lint-tab *Open Infra — GIS***, ingedeeld zoals de werkbalken van de FreeCAD GIS-workbench:
  - *Webservices*: Geo-locatie, WMS/TMS, WFS, OpenStreetMap, PDOK (NL).
  - *Snel importeren*: kaartbeeld en vectordata met één klik.
  - *Hulpmiddelen*: Gebied, Punt info, Coördinaten, Servers.
- **Werkbankvenster** naar het voorbeeld van de FreeCAD GIS-dialogen:
  - Bovenaan een **kaart** met de projectlocatie en de begrenzing.
  - De achtergrondkaart kies je rechtsboven met een Leaflet-lagenkiezer: OSM, PDOK BRT, PDOK-luchtfoto of Sentinel-2.
  - Klik op de kaart om de locatie te kiezen; teken het gebied met Shift+slepen.
  - **Test** toont een voorbeeld van de import op de kaart; **OK en importeren** plaatst het in de tekening.
- **PDOK (NL)** in één keer: luchtfoto, kadastrale kaart, AHN, BAG, percelen, NWB-wegvakken en -hectometerpunten, ProRail-spoorassen en -kilometrering, vaarwegen.
- **Coördinatenstelsels:**
  - Ingebouwd: RD New, ETRS89/UTM, België, VK, Zwitserland en Frankrijk.
  - Wereldwijd automatisch UTM, plus elke EPSG-code via epsg.io.
  - De adreszoeker stelt het landelijke stelsel voor.
- **Imports:**
  - WMS met tegelopdeling en herprojectie, en XYZ-tegels.
  - WFS met paging.
  - OpenStreetMap via Overpass.
  - Alles kan worden bijgesneden op het importgebied.
- **In de tekening:**
  - De projectlocatie wordt in de DWG opgeslagen; standaard is dat de Grote Kerk in Dordrecht.
  - Optioneel een basispunt-markering (GeoOrigin).
  - Kaartbeelden staan naast de tekening en achter de vectoren.
  - Elke import is één ongedaan-maakstap.

Het venster werkt op Linux en Windows. Op macOS werken de lintknoppen en commando's, maar het venster nog niet.

Data: PDOK, Kadaster, Rijkswaterstaat en ProRail (open data). © OpenStreetMap-bijdragers (ODbL). Sentinel-2 cloudless by EOX (CC BY-NC-SA 4.0).
