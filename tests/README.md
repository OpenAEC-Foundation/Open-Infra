# Testset Open Infra

Drie lagen tests. De locaties en hun verwachtingen staan in [`data/locaties.json`](data/locaties.json):

- Nederland: Dordrecht, Amsterdam, Utrecht, de Maasvlakte, een polder bij Lelystad, de Vaalserberg, Terschelling en Loppersum.
- Over de grens: Antwerpen, Aken, Londen, Zürich, Parijs, New York en Sydney.
- De Noordzee: overal leeg.
- 30 grenspunten rond Nederland.

## 1. Offline (ook in CI)

```bash
cargo test --lib            # eenheidstests van de engine
cargo test --test locaties  # locatieset zonder netwerk
```

| Test | Wat |
|---|---|
| `stelsel_per_locatie` | Het voorgestelde coördinatenstelsel per locatie, op land (zoeken) en op positie (klik op de kaart). |
| `stelselkeuze_rond_de_grens` | 30 plaatsen rond de Nederlandse grens: Kerkrade RD, Aken UTM 32, Hulst RD, Antwerpen Lambert 72, Baarle-Nassau, Lille, enzovoort. |
| `transformatie_heen_en_terug` | Heen en terug binnen 1 cm, en de schaal van het stelsel. |
| `projectlocatie_in_de_tekening` | Het importgebied in de tekening: absoluut en relatief, meters en mm. |
| `bgt_kenmerken_naar_nlcs` | 121 echte BGT-kenmerkcombinaties ([`data/bgt-kenmerken.json`](data/bgt-kenmerken.json)). Elk object moet op een geldige NLCS-laag landen. |
| `klic_levering_naar_nlcs_en_imkl` | De KLIC-voorbeeldlevering van GIS2BIM: NLCS-objecten, IMKL-kleuren, ligging en labels. Wordt overgeslagen als de levering er niet is (`OPEN_INFRA_KLIC=<zip>`). |
| `hoogtelijnen_en_profiel_op_een_dijk` | Een kunstmatige dijk met een gat. Gecontroleerd: de ligging van de hoogtelijnen en de profielstations. |
| `tekenblad_kiest_het_formaat` | A3–A0 bij 1:500 en 1:1000. |
| `nap_notatie` | `+2,43`, `-6,10`. |

## 2. Online: data ophalen per locatie

```bash
cargo test --test locaties -- --ignored --nocapture
LOCATIES=dordrecht,vaals cargo test --test locaties -- --ignored --nocapture
```

Per locatie worden deze bronnen getest:
- zoeken (PDOK en Nominatim);
- OSM-kaartbeeld en PDOK-luchtfoto;
- BGT: actueel, binnen het gebied, alles NLCS;
- BRK;
- OpenStreetMap;
- AHN-hoogtes, binnen het verwachte NAP-bereik, en hoogtelijnen;
- AHN-profiel;
- AHN-puntenwolk, met een `.rcs`-bestand;
- het formaat van het tekenblad.

Buiten Nederland moeten BGT, BRK en AHN leeg blijven of netjes weigeren, zonder te crashen.

Het rapport staat in `target/locatietest/rapport.md`, met een matrix locatie × bron, en in `rapport.json`. Een overbelaste openbare dienst (Overpass, Nominatim) telt als waarschuwing ⚠, niet als fout ✘.

In GitHub Actions draait deze test elke maandag. Hij is ook op verzoek te starten (*Actions → test → Run workflow*); het rapport is een artifact.

## 3. End-to-end in Open CAD Studio

```bash
./scripts/install.sh                       # plugin installeren
python3 tests/e2e/ocs_e2e.py               # Dordrecht, Vaals, Lelystad, Londen
python3 tests/e2e/ocs_e2e.py --snel        # alleen Dordrecht
```

Het script start Open CAD Studio met een lege tekening en de REST-koppeling (`OpenCADStudio start.dwg --http <poort>`), met de geïnstalleerde plugin. Pad naar de executable: `--ocs` of `OCS_BIN`.

Er verschijnt dus een venster. Zonder venster verwerkt Open CAD Studio de achtergrondverzoeken van een plugin alleen tijdens een plugin-commando, waardoor imports stil blijven staan. Daarom draait deze test niet in CI.

| Scenario | Wat |
|---|---|
| importeren | Projectlocatie (`GIS_SITE`) en de imports: BGT, kadaster, AHN-hoogtelijnen en -punten, of OSM buiten Nederland. Daarna een tekenblad. Gecontroleerd: NLCS-lagen, elementen binnen het importgebied en XDATA. |
| bewerken | Een BGT-object verplaatsen (de XDATA blijft), het naar een andere laag zetten en een laag leegmaken. Ongedaan maken en opnieuw. Een hele import in één stap ongedaan maken. Opnieuw importeren. |
| opslaan | Opslaan en een kopie heropenen: gelijke elementen, lagen en XDATA. |
| eenheden | Een mm-tekening (`data/leeg-mm.dwg`): alles is 1000 × zo groot. |
| relatief | `GIS_SITE … RELATIEF`: het basispunt ligt op 0,0. |
| verhuizen | Een tweede locatie in dezelfde tekening. |

Het rapport staat in `target/e2e/rapport.md`; de tekeningen staan in `target/e2e/`.

## Wat de testset tot nu toe vond

In de plugin, opgelost:
- **Coördinatenstelsel op positie:** Antwerpen, Gent, Brugge, Aken, Kleve, Emmerich en Gronau kregen RD, Lille Lambert 72 en de Noordzee RD. Nu worden grove landsgrenzen voor NL en BE gebruikt.
- **Zoeken:** "Grote Kerk Dordrecht" gaf een straat in Hoorn, "Utrecht Centraal" een busstation in Breda. Nu staan treffers met alle zoekwoorden voorop, en als die er niet zijn wordt in de laatst genoemde plaats gezocht.
- **mm- en cm-tekeningen:** de plugin las INSUNITS niet, want Open CAD Studio geeft die niet als systeemvariabele. Daardoor kwam alles in meters. Nu komt de eenheid uit de tekeningkop.
- **AHN-puntenwolk:** AHN5 is nog niet compleet (bijvoorbeeld bij Loppersum). Een ontbrekende tegel komt nu uit AHN4.
- **AHN buiten Nederland:** gaf een kale WCS-fout. Nu volgt vooraf een duidelijke melding.
- **IMKL-kleuren:** "buisleiding gevaarlijke inhoud" had de kleur van petrochemie; "wees" en "overig" ontbraken. Ze volgen nu de PMKL-handreiking 3.0.

In Open CAD Studio v2026.42 (host), nog open:
- **Ongedaan maken:** een import van de plugin is niet ongedaan te maken. Elementen die een plugin op de achtergrond toevoegt komen niet in de ongedaan-maakstap.
- **Zonder venster** (`--http` zonder tekening) verwerkt Open CAD Studio de achtergrondverzoeken van een plugin alleen tijdens een plugin-commando.
- **REST:** de routes `/save` en `/undo` nemen geen `document_id` mee in de GUI-modus. De test gebruikt daarom `QSAVE` en `U`.

Gedrag om te beslissen:
- **Opnieuw importeren** in hetzelfde gebied voegt alles nogmaals toe (dubbele elementen).

