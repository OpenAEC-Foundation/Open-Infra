Open Infra — GIS-workbench en NLCS-basisdata voor **Open CAD Studio v2026.42** (plugin-API 10), voor infra en civiele techniek in Nederland en wereldwijd.

![Voorbeeld: situatie bestaand rond de Grote Kerk in Dordrecht, A1 1:500](https://raw.githubusercontent.com/OpenAEC-Foundation/Open-Infra/v2026.43/examples/dordrecht-nlcs/dordrecht-grote-kerk-nlcs.png)

**Installeren:** in Open CAD Studio `PLUGINS` → *Add a repository* → `OpenAEC-Foundation/Open-Infra`. Installeer deze release en herstart.
Handmatig kan ook: zet `openaec.open-infra-<platform>` en `plugin.toml` in `<config>/OpenCADStudio/plugins/openaec.open-infra/`.

**Nieuw in 2026.43 — Basisdata NL conform NLCS**

Vanaf deze release volgen de versienummers de kalendernummering van Open CAD Studio. Release 2026.43 is gebouwd voor Open CAD Studio v2026.42.

- **NLCS 5.02 ingebouwd:**
  - 3.659 objecten, 256 kleuren en 543 lijntypen.
  - Alle statussen en disciplines.
  - De officiële NLCS-BGT 5.0-mapping met 719 regels (DigiGO / Stichting NLCS, CC BY 4.0).
- **Lagen:** elk object komt op zijn NLCS-laag `STATUS-DISCIPLINE-HOOFDGROEP-OBJECT-ELEMENT`.
  - De laag krijgt de NLCS-kleur, -lijndikte en -lijntype.
  - Lijntypen worden in papier-mm gemaakt; de lijntypeschaal volgt de tekenschaal (1:100 – 1:5000).
- **BGT:** de actuele objecten via de PDOK OGC API, in 42 collecties en 11 groepen. Straatnamen en huisnummers krijgen de draaiing uit de BGT.
- **Kadastrale kaart (BRK):** perceelgrenzen en perceelnummers, op `KG-GRENS_PERCEEL`.
- **KLIC:**
  - De levering wordt ingelezen als zip of GI-xml (IMKL 2.0, met `klic2geo`).
  - 13 thema's op de NLCS-objecten KL en RI.
  - Labels per kabel of leiding, zoals `G-LD Stedin 160 mm PVC`.
  - Optioneel de IMKL-themakleuren.
- **Objectgegevens:** elk object krijgt zijn bronattributen als XDATA `OPENINFRA`.
- **Tekenblad:**
  - Kader en titelblok, met de bronnen van de imports.
  - Een legenda van de gebruikte NLCS-lagen.
  - Noordpijl, schaalbalk en RD-kruisen.
  - Het formaat wordt automatisch gekozen (A3–A0).
- **Werkbank:** nieuwe pagina *BGT · Kadaster · KLIC* met **Test** (voorbeeld op de kaart) en **OK en importeren**. Een KLIC-levering kun je ook op het venster slepen.
- **Lint:** nieuwe groep *Basisdata NL (NLCS)* met BGT · BRK · KLIC, BGT, Kadaster, KLIC (bestandskiezer) en Tekenblad.
- **Nieuwe commando's:** `GIS_SITE`, `GIS_BGT`, `GIS_Q_BGT`, `GIS_Q_BRK`, `GIS_KLIC`, `GIS_KLIC_IMPORT` en `GIS_TEKENBLAD`.
- **Voorbeeldtekening** in `examples/dordrecht-nlcs`: de Grote Kerk in Dordrecht met BGT, kadaster en KLIC, A1 1:500, als DWG, PDF en PNG.

Beperkingen:
- Lijntypen met symbolen of tekst staan er alleen als streepjespatroon in.
- Puntobjecten zijn cirkels in plaats van NLCS-symbolen.
- Vlakken krijgen nog geen arcering.

**Uit 0.2.1**
- **Groter locatiescherm:** het werkbankvenster opent op 1280 × 1000 en de kaart op de pagina *Geografische locatie* vult het grootste deel van het venster, zoals de kaart in de FreeCAD-dialoog "Set Geographic Location".

**Uit 0.2.0**
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
