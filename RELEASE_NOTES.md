Open Infra — GIS, NLCS en civiel ontwerp voor **Open CAD Studio v2026.43** (plugin-API 13), voor infra en civiele techniek in Nederland en wereldwijd. Twee plugins: het tabblad **GIS** en het nieuwe tabblad **Civiel**.

**Let op:** deze release is gebouwd voor Open CAD Studio **v2026.43**. Een plugin voor v2026.42 laadt niet in v2026.43, en andersom; werk Open Infra dus samen met Open CAD Studio bij.

**Nieuw — tabblad Civiel (Civil 3D-achtig, naar Hakans FreeCAD Road-workbench)**

- **Punten:**
  - meetpunten uit PNEZD/PENZD/CSV, met scheidingsteken, decimale komma en N/E-volgorde automatisch herkend;
  - punten plaatsen met de hoogte uit TIN of AHN;
  - exporteren;
  - LandXML (punten, oppervlakken, assen met bogen en clothoïden).
- **Oppervlak:**
  - TIN uit punten, 3D-lijnen, hoogtelijnen of AHN, met status B (bestaand) of N (ontwerp);
  - hoogtelijnen op de TIN;
  - hoogteanalyse in kleurbanden;
  - grondverzet: ontwerp tegen bestaand, of tegen een peil.
- **As:**
  - uit een polylijn, of uit PI-punten met boogstralen;
  - stationering (0+100), raaklijnpunten BB/EB/HP en boogstralen;
  - verschoven lijnen;
  - station en offset van punten.
- **Profielen:**
  - lengteprofiel met boogband;
  - ontwerpprofiel uit PVI's met parabolische top- en voetbogen (L en R) en hellingen;
  - dwarsprofielen in een raster.
- **Corridor:**
  - sjablonen weg, fietspad, snelweg en dijk; rijstrook, berm, hellingen, taluds, kruin en verkanting met uitrol zijn instelbaar;
  - aansluiting op het maaiveld;
  - ontwerp-TIN en 3D-kantlijnen;
  - afgraven en ophogen per dwarsprofiel (gemiddelde-eindvlakmethode) met massabalans (CSV);
  - dwarsprofielen met het ontwerp erin.
- **3D-model:**
  - het AHN-terrein gekleurd met de PDOK-luchtfoto;
  - 3D BAG-panden in LoD 1.2, 1.3 of 2.2;
  - de hele BGT in 3D: vlakken gedrapeerd, gebouwen en muren geëxtrudeerd, water op peil, bruggen op dekhoogte, palen en bomen als 3D-symbool, in natuurlijke kleuren.

**Nieuw — API en MCP-server**

- **Argumenten:** elk `CIV_`-commando en de GIS-datacommando's werken ook met argumenten (`SLEUTEL=waarde`, punten `x,y`). Met `ID=<naam>` staat het resultaat als JSON in `<config>/OpenCADStudio/open-infra/api/` (handles, lengtes, volumes).
- **MCP-server** (`mcp/open_infra_mcp.py`): ongeveer 30 tools waarmee een AI-agent (Claude Code, Claude Desktop) hele modellen bouwt. Hij vindt elk Open CAD Studio-venster vanzelf.
- **Getest met een megamodel van Dordrecht-binnenstad** (1,2 × 1 km), via de MCP-server gebouwd:
  - 299.000 fotocellen;
  - 18.560 BGT-objecten in 3D;
  - 2.864 panden in LoD 2.2;
  - een rondweg en een dijk met corridor en dwarsprofielen.

**Verder**

- **NLCS-controle en hoeveelheden** (ook in 2026.44.x-beta's):
  - de controle in een eigen pagina, met herstel (ongedaan te maken) en een rapport;
  - hoeveelheden per laag of object, als CSV of als staat in de tekening.
- **GIS-lint:** de Nederlandse PDOK-onderdelen staan compact in één groep *Nederland (PDOK)*, als menu's. De wereldwijde kaart- en datatools krijgen de ruimte.
- **Grote tekeningen:**
  - de plugin leest ze via de gedeelde geheugenweergave van Open CAD Studio;
  - elementen gaan in delen onder de berichtgrens van 64 MB;
  - 3D-objecten zijn lichte meshes.

**Uit 2026.44.2 — overzichtelijk zoeken bij Geo-locatie**

- **Suggesties tijdens het typen:** na een korte pauze verschijnt een lijst met Nederlandse adressen (PDOK). Enter of **Zoeken** zoekt ook wereldwijd (OpenStreetMap).
- **Elke treffer op twee regels:** de naam vet, eronder de plaats ("3311 CJ Dordrecht"). Een gekleurd label toont het soort treffer: adres, straat, plaats, perceel of coördinaat. Bij treffers uit beide bronnen zijn ze gegroepeerd onder *Nederland · PDOK* en *Wereldwijd · OpenStreetMap*.
- **Geen rommel:**
  - Geen dubbele treffers meer (dezelfde straat als straat, postcode én OSM).
  - Zonder huisnummer één regel per straat.
  - Losse, niet-passende treffers vallen weg.
  - Een exacte treffer staat bovenaan.
  - Wereldwijde namen zijn kort en in het Nederlands.
- **Bediening:** pijltjes en Enter om te kiezen, Esc om te sluiten. "Zoeken…" en "Niets gevonden" staan in de lijst zelf.
- **RD-coördinaten** (`104885 425320`) worden herkend, naast breedte- en lengtegraad.
- **De lijst bedekt het zoekveld nooit:** hij opent boven het veld als er onder geen ruimte is.

**Uit 2026.44.1 — testset en reparaties**

Een nieuwe testset haalt data op op 16 locaties, van Dordrecht en de Vaalserberg tot Antwerpen, Londen, New York en de Noordzee. Hij bewerkt de tekening ook in Open CAD Studio. Dit vond hij, en dit is opgelost:

- **Coördinatenstelsel bij een klik op de kaart:** Antwerpen, Gent, Brugge, Aken, Kleve, Emmerich en Gronau kregen RD, Lille Lambert 72 en de Noordzee RD. Nu worden grove landsgrenzen van Nederland en België gebruikt; 30 grensplaatsen worden getest.
- **Zoeken:** "Grote Kerk Dordrecht" gaf een straat in Hoorn, "Utrecht Centraal" een busstation in Breda. Nu staan treffers met alle zoekwoorden voorop, en anders wordt in de laatst genoemde plaats gezocht.
- **mm- en cm-tekeningen** kregen alles in meters: de plugin las INSUNITS niet. Nu komt de eenheid uit de tekening.
- **AHN-puntenwolk:** waar AHN5 nog ontbreekt (bijvoorbeeld in Groningen), komt de tegel uit AHN4.
- **AHN buiten Nederland** geeft nu een duidelijke melding in plaats van een serverfout.
- **KLIC IMKL-themakleuren** volgen nu precies de PMKL-handreiking 3.0: buisleiding gevaarlijke inhoud #ff7f00, plus wees en overig.
- **`GIS_SITE … RELATIEF`** zet de projectlocatie met het basispunt op 0,0.

Testset ([tests/README.md](https://github.com/OpenAEC-Foundation/Open-Infra/blob/main/tests/README.md)):
- **Offline:** bij elke push op GitHub.
- **Online:** wekelijks, met per locatie zoeken, kaartbeeld, BGT, BRK, OSM, AHN, profiel en puntenwolk.
- **End-to-end in Open CAD Studio:** importeren, verplaatsen, laag wijzigen, wissen, ongedaan maken, opslaan en heropenen, mm-tekening, relatieve coördinaten.

Bekend, in Open CAD Studio v2026.42 zelf:
- Een import van de plugin is niet ongedaan te maken. Wat oudere release-notes beloofden ("elke import is één ongedaan-maakstap") klopt dus niet.
- Opnieuw importeren in hetzelfde gebied voegt alles nogmaals toe.

**Uit 2026.44 — AHN-hoogtes, profielen en puntenwolk**

![AHN5-puntenwolk in Open CAD Studio](https://raw.githubusercontent.com/OpenAEC-Foundation/Open-Infra/v2026.44.2/docs/images/ahn-pointcloud.png)

- **Hoogtepunten:** AHN4-maaiveld (DTM) of -oppervlak (DSM) in een raster van 2 tot 100 m.
  - Ze staan op `OG-HOOGTEPUNT`, met hoogtes als `+2,43` m NAP.
- **Hoogtelijnen:** uit het AHN4-maaiveld.
  - Interval naar keuze (0,1 – 5 m), met gelabelde hoofdlijnen op `OG-HOOGTELIJN_MINOR/MAJOR`.
  - Licht gladgestreken en zonder snippers.
  - Lijnen en punten liggen op hun hoogte (Z).
- **Lengteprofiel** (`GIS_PROFIEL`):
  - Selecteer een polyline of wijs punten aan, druk op Enter en wijs de plek van het profiel aan.
  - Het profiel toont de maaiveldlijn (`GW-MAAIVELD`) en optioneel de oppervlaklijn (DSM), met een hoogteraster en NAP-labels.
  - Een profielbalk geeft afstanden en hoogtes; de NAP-lijn verschijnt als die binnen het bereik valt.
  - In de situatie komt doorsnedelijn A–A'.
  - De overhoging is instelbaar.
- **Puntenwolk** (`GIS_PUNTENWOLK`):
  - AHN5 of AHN4 als COPC. Alleen het nodige deel van de tegels van 1 km² wordt gedownload, met een puntafstand naar keuze.
  - Kleur naar klasse, hoogte of RGB, en een filter op klassen (maaiveld, gebouwen, water, kunstwerken, overig).
  - De puntenwolk wordt opgeslagen als `.rcs` naast de tekening en gekoppeld; je ziet hem in een 3D-weergavestijl.
- **Punt info** geeft nu ook de AHN-maaiveld- en oppervlakhoogte.
- **Werkbank:** pagina *AHN-hoogtes* met **Test** (hoogtelijnen op de kaart). In het lint de groep *Hoogte (AHN)* met AHN, Hoogtepunten, Hoogtelijnen, Profiel en Puntenwolk.
- **Nieuwe schermafbeeldingen** in de README.

Data:
- AHN4-hoogtegrids via de PDOK-WCS.
- AHN4/AHN5-puntenwolken van Het Waterschapshuis.
- Het AHN is open data (CC0).

**Uit 2026.43 — Basisdata NL conform NLCS**

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
